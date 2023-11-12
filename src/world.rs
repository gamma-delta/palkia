//! The place all the entities, resources, and components live, at the heart of your project.

use std::collections::{BTreeMap, BTreeSet};

use crossbeam::channel;

use crate::{
  access::{AccessDispatcher, AccessEntityStats, AccessQuery, AccessResources},
  builder::EntityBuilder,
  callback::{
    CallbackWorldAccess, Callbacks, OnCreateCallback, OnRemoveCallback,
  },
  component::{Component, HandlerBuilder},
  entities::{Entity, EntityAssoc, EntityIter, EntityLiveness, EntityStorage},
  loop_panic,
  messages::{ListenerWorldAccess, Message, MsgHandlerInner},
  prelude::Query,
  resource::{
    ReadResource, Resource, ResourceLookupError, ResourceMap, WriteResource,
  },
  ToTypeIdWrapper, TypeIdWrapper,
};

pub struct World {
  /// Each entity maps type IDs to their components
  pub(crate) entities: EntityStorage,
  /// Maps event types to, maps component types to the EventHandler.
  msg_table: BTreeMap<TypeIdWrapper, BTreeMap<TypeIdWrapper, MsgHandlerInner>>,
  pub(crate) known_component_types: BTreeSet<TypeIdWrapper>,

  pub(crate) resources: ResourceMap,

  pub(crate) lazy_sender: channel::Sender<LazyUpdate>,
  lazy_channel: channel::Receiver<LazyUpdate>,

  /// Maps component types to their callbacks
  callbacks: BTreeMap<TypeIdWrapper, Callbacks>,
}

impl World {
  pub fn new() -> World {
    let (tx, rx) = channel::unbounded();

    Self {
      entities: EntityStorage::default(),
      msg_table: BTreeMap::new(),
      resources: ResourceMap::new(),
      known_component_types: BTreeSet::new(),
      lazy_sender: tx,
      lazy_channel: rx,
      callbacks: BTreeMap::new(),
    }
  }

  /// Register a component type to the world.
  ///
  /// Panics if that component type has already been registered.
  pub fn register_component<C: Component>(&mut self) {
    if !self.known_component_types.insert(TypeIdWrapper::of::<C>()) {
      panic!(
        "already registered component type {:?}",
        TypeIdWrapper::of::<C>().type_name
      );
    }

    let builder = HandlerBuilder::<C>::new();
    let builder = C::register_handlers(builder);
    for (ev_type, handler) in builder.handlers {
      self
        .msg_table
        .entry(ev_type)
        .or_default()
        .insert(TypeIdWrapper::of::<C>(), handler);
    }

    let cbs = mux_callbacks(builder.create_cb, builder.remove_cb);
    if let Some(cbs) = cbs {
      self.callbacks.insert(TypeIdWrapper::of::<C>(), cbs);
    }
  }

  /// Extend a component type by adding more message handlers to it. (Perhaps the component was defined in
  /// another crate.)
  ///
  /// The closure will receive a [`HandlerBuilder`]; use it as you might in the original closure implementation.
  /// If the component previously defined a handler for a message, the one added here will clobber the old one.
  /// Duplicate callbacks are currently not implemented and will panic.
  ///
  /// Panics if that component type has not been registered yet.
  pub fn extend_component<C: Component>(
    &mut self,
    extension: impl FnOnce(HandlerBuilder<C>) -> HandlerBuilder<C>,
  ) {
    let tid = TypeIdWrapper::of::<C>();
    if !self.known_component_types.contains(&tid) {
      panic!(
        "tried to extend unregistered component type {:?}",
        tid.type_name
      );
    }

    let builder = HandlerBuilder::<C>::new();
    let builder = extension(builder);

    if builder.create_cb.is_some() || builder.remove_cb.is_some() {
      panic!(
        "tried to extend component type {:?} by adding create/remove callbacks",
        tid.type_name
      );
    }

    for (ev_type, handler) in builder.handlers {
      self
        .msg_table
        .entry(ev_type)
        .or_default()
        .insert(TypeIdWrapper::of::<C>(), handler);
    }
  }

  /// Set up a builder to spawn an entity with a whole mess of components.
  pub fn spawn<'w>(&'w mut self) -> EntityBuilder<'w, 'w> {
    let to_create = self.entities.spawn_unfinished();
    EntityBuilder::new_immediate(self, to_create)
  }

  /// Spawn a new empty entity.
  pub fn spawn_empty(&mut self) -> Entity {
    // no need to run callbacks cause there's nothing on it to call back
    self.entities.spawn(EntityAssoc::empty())
  }

  /// As a convenience method, spawn an entity with a single component.
  pub fn spawn_1<C: Component>(&mut self, component: C) -> Entity {
    let assoc = EntityAssoc::new([Box::new(component) as _]);
    let e = self.entities.spawn(assoc);
    self.run_creation_callbacks(e);
    e
  }

  /// Set up a builder to spawn an entity once `finalize` has been called.
  ///
  /// The advantage of this is it doesn't need mutable access.
  pub fn lazy_spawn<'w>(&'w self) -> EntityBuilder<'w, 'w> {
    let entity = self.entities.spawn_unfinished();
    EntityBuilder::new_lazy_world(self, entity)
  }

  /// Despawn an entity immediately. Panics if the entity does not exist.
  pub fn despawn(&mut self, entity: Entity) {
    self.entities.despawn(entity);
  }

  /// Lazily despawn an entity immediately; it will be removed once [`World::finalize`] is called.
  ///
  /// Panics if the entity does not exist.
  pub fn lazy_despawn(&self, entity: Entity) {
    self
      .lazy_sender
      .send(LazyUpdate::DespawnEntity(entity))
      .unwrap();
  }

  /// Convenience method to dispatch a message to all entities, cloning it for each entity.
  pub fn dispatch_to_all<M: Message + Clone>(&self, msg: M) {
    for e in self.entities.iter() {
      self.dispatch(e, msg.clone());
    }
  }

  /// Insert a resource into the world, returning the old value if it existed.
  pub fn insert_resource<R>(&mut self, resource: R) -> Option<R>
  where
    R: Resource,
  {
    self.resources.insert(resource)
  }

  /// Insert a resource with a default into the world, returning the old value if it existed.
  pub fn insert_resource_default<R>(&mut self) -> Option<R>
  where
    R: Resource + Default,
  {
    self.resources.insert(R::default())
  }

  /// With ownership, get direct mutable access to the given resource.
  pub fn get_resource<R: Resource>(&mut self) -> Option<&mut R> {
    self.resources.get()
  }

  /// With ownership, remove and return the given resource
  pub fn remove_resource<R: Resource>(&mut self) -> Option<R> {
    self.resources.remove()
  }

  /// Apply any and all lazy updates.
  pub fn finalize(&mut self) {
    let updates = self.lazy_channel.try_iter().collect::<Vec<_>>();
    for lazy in updates {
      lazy.apply(self);
    }
  }

  /// Get an iterator over all the entities in the world.
  ///
  /// You *probably* don't want to use this; try [`World::dispatch_to_all`] instead.
  pub fn entities(&self) -> EntityIter<'_> {
    self.entities.iter()
  }

  pub(crate) fn run_creation_callbacks(&self, e: Entity) {
    let access = CallbackWorldAccess::new(self);
    for (tid, comp) in self.entities.get(e).components() {
      if let Some(cb) = self.callbacks.get(tid).and_then(Callbacks::get_create)
      {
        // i am *pretty* sure this will never be locked?
        let comp = comp.try_read().unwrap();
        cb(comp.as_ref(), e, &access);
      }
    }
  }
  pub(crate) fn run_removal_callback(&self, e: Entity, comps: EntityAssoc) {
    let access = CallbackWorldAccess::new(self);
    for (tid, comp) in comps.into_iter() {
      if let Some(cb) = self.callbacks.get(&tid).and_then(Callbacks::get_remove)
      {
        let comp = comp.into_inner().unwrap();
        cb(comp, e, &access);
      }
    }
  }

  #[doc(hidden)]
  pub fn dump(&self) {
    println!("Callbacks:");
    for (tid, cb) in self.callbacks.iter() {
      println!(
        " {}: {}",
        tid.type_name,
        match cb {
          Callbacks::Create(..) => "create",
          Callbacks::Remove(..) => "remove",
          Callbacks::Both(..) => "create/remove",
        }
      );
    }
  }
}

impl AccessDispatcher for World {
  fn dispatch<M: Message>(&self, target: Entity, msg: M) -> M {
    dispatch_inner(&ListenerWorldAccess::new(self), target, msg)
  }
}

impl AccessEntityStats for World {
  fn len(&self) -> usize {
    self.entities.len()
  }

  fn liveness(&self, entity: Entity) -> EntityLiveness {
    self.entities.liveness(entity)
  }

  fn len_of(&self, entity: Entity) -> usize {
    self.entities.len_of(entity)
  }

  fn iter(&self) -> crate::entities::EntityIter<'_> {
    self.entities.iter()
  }
}

impl AccessQuery for World {
  fn query<'c, Q: Query<'c>>(
    &'c self,
    interrogatee: Entity,
  ) -> Option<Q::Response> {
    let comps = self.entities.get(interrogatee);
    Q::query(interrogatee, comps)
  }
}

impl AccessResources for World {
  fn read_resource<R: Resource>(
    &self,
  ) -> Result<ReadResource<'_, R>, ResourceLookupError> {
    self.resources.read()
  }

  fn write_resource<R: Resource>(
    &self,
  ) -> Result<WriteResource<'_, R>, ResourceLookupError> {
    self.resources.write()
  }

  fn contains_resource<R: Resource>(&self) -> bool {
    self.resources.contains::<R>()
  }
}

pub(crate) enum LazyUpdate {
  FinishEntity(Vec<Box<dyn Component>>, Entity),
  DespawnEntity(Entity),
}

impl LazyUpdate {
  fn apply(self, world: &mut World) {
    match self {
      LazyUpdate::FinishEntity(comps, entity) => {
        world.entities.finish_spawn(entity, EntityAssoc::new(comps));
        world.run_creation_callbacks(entity);
      }
      LazyUpdate::DespawnEntity(entity) => {
        if world.entities.liveness(entity) == EntityLiveness::Alive {
          let prev = world.entities.despawn(entity);
          world.run_removal_callback(entity, prev);
        }
        // Otherwise, it was double-killed, we hope
      }
    }
  }
}

pub(crate) fn dispatch_inner<M: Message>(
  access: &ListenerWorldAccess,
  target: Entity,
  msg: M,
) -> M {
  let msg2 = dispatch_even_innerer(access, target, Box::new(msg));
  // SAFETY: the type ID guards prevent this from being different
  unsafe { *msg2.downcast().unwrap_unchecked() }
}

pub(crate) fn dispatch_even_innerer(
  access: &ListenerWorldAccess,
  target: Entity,
  mut msg: Box<dyn Message>,
) -> Box<dyn Message> {
  let event_handlers =
    match access.world.msg_table.get(&(*msg).type_id_wrapper()) {
      Some(it) => it,
      None => {
        // i've never met this event type in my life
        return msg;
      }
    };

  let components = access.world.entities.get(target);
  for (tid, comp) in components.iter() {
    if let Some(handler) = event_handlers.get(&tid) {
      let lock = comp.try_read().unwrap_or_else(|_| loop_panic(target, tid));
      let msg2 = match handler {
        MsgHandlerInner::Read(handler) => handler(&**lock, msg, target, access),
        MsgHandlerInner::Write(handler) => {
          drop(lock);
          let mut lock =
            comp.try_write().unwrap_or_else(|_| loop_panic(target, tid));
          handler(&mut **lock, msg, target, access)
        }
      };
      msg = msg2;
      if access.is_cancelled() {
        break;
      }
    }
  }

  for (queued_msg, target) in access.queued_message_rx().try_iter() {
    dispatch_even_innerer(access, target, queued_msg);
  }

  msg
}

fn mux_callbacks(
  create: Option<OnCreateCallback>,
  remove: Option<OnRemoveCallback>,
) -> Option<Callbacks> {
  match (create, remove) {
    (None, None) => None,
    (None, Some(remove)) => Some(Callbacks::Remove(remove)),
    (Some(create), None) => Some(Callbacks::Create(create)),
    (Some(create), Some(remove)) => Some(Callbacks::Both(create, remove)),
  }
}

/// Information stored about each component.
pub(crate) struct ComponentVtable {
  pub tid: TypeIdWrapper,
  /// Used for ser/de, both from kdl and to disc
  pub friendly_name: &'static str,
  /// Maps event types to msg handlers
  pub msg_table: BTreeMap<TypeIdWrapper, MsgHandlerInner>,
}
