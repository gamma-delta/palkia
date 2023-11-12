//! The place all the entities, resources, and components live, at the heart of your project.

pub(crate) mod storage;
// public for the benefit of `Query`
#[doc(hidden)]
pub use storage::EntityAssoc;

use std::collections::BTreeMap;

use crossbeam::channel;

use crate::{
  access::{AccessDispatcher, AccessEntityStats, AccessQuery, AccessResources},
  builder::EntityBuilder,
  callback::CallbackWorldAccess,
  component::{Component, ComponentRegisterer},
  entities::{Entity, EntityIter, EntityLiveness},
  loop_panic,
  messages::{ListenerWorldAccess, Message, MsgHandlerInner},
  prelude::Query,
  resource::{
    ReadResource, Resource, ResourceLookupError, ResourceMap, WriteResource,
  },
  vtablesathome::ComponentVtable,
  ToTypeIdWrapper, TypeIdWrapper,
};

use self::storage::EntityStorage;

pub struct World {
  /// Each entity maps type IDs to their components
  pub(crate) entities: EntityStorage,
  pub(crate) components: BTreeMap<TypeIdWrapper, ComponentVtable>,

  pub(crate) resources: ResourceMap,

  pub(crate) lazy_sender: channel::Sender<LazyUpdate>,
  lazy_channel: channel::Receiver<LazyUpdate>,
}

impl World {
  pub fn new() -> World {
    let (tx, rx) = channel::unbounded();

    Self {
      entities: EntityStorage::default(),
      components: BTreeMap::new(),
      resources: ResourceMap::new(),
      lazy_sender: tx,
      lazy_channel: rx,
    }
  }

  /// Register a component type to the world.
  ///
  /// Panics if that component type has already been registered.
  pub fn register_component<C: Component>(&mut self) {
    let tid = TypeIdWrapper::of::<C>();
    if self.components.contains_key(&tid) {
      panic!("already registered component type {:?}", tid.type_name);
    }

    let blank_builder = ComponentRegisterer::<C>::new();
    let builder = C::register(blank_builder);
    self.components.insert(tid, builder.into_vtable());
  }

  /// Set up a builder to spawn an entity with a whole mess of components.
  pub fn spawn<'w>(&'w mut self) -> EntityBuilder<'w, 'w> {
    let to_create = self.entities.spawn_unfinished();
    EntityBuilder::new_immediate(self, to_create)
  }

  /// As a helper method (mostly for tests) spawn an entity with just
  /// 1 component.
  pub fn spawn_1<C: Component>(&mut self, comp: C) -> Entity {
    self.spawn().with(comp).build()
  }

  /// As a helper method (mostly for tests) spawn an entity with 0 components.
  ///
  /// It will literally do nothing.
  pub fn spawn_empty(&mut self) -> Entity {
    self.spawn().build()
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

  /// Returns if the given component type has been registered.
  pub fn knows_component<C: Component>(&self) -> bool {
    self.knows_component_tid(TypeIdWrapper::of::<C>())
  }

  #[doc(hidden)]
  pub fn knows_component_tid(&self, tid: TypeIdWrapper) -> bool {
    self.components.contains_key(&tid)
  }

  /// Finish the spawning of an entity that's been lazily created but not
  /// instantiated fully.
  ///
  /// Panics if the invariant is not upheld.
  pub(crate) fn finish_spawn(&mut self, target: Entity, assoc: EntityAssoc) {
    #[cfg(debug_assertions)]
    {
      for (cmp_tid, _) in assoc.iter() {
        if !self.knows_component_tid(cmp_tid) {
          panic!(
            "tried to spawn an entity with the unregistered type {}",
            cmp_tid.type_name
          );
        }
      }
    }

    self.entities.finish_spawn(target, assoc);
    self.run_creation_callbacks(target);
  }

  pub(crate) fn run_creation_callbacks(&self, e: Entity) {
    let access = CallbackWorldAccess::new(self);
    for (tid, comp) in self.entities.get(e).components() {
      if let Some(cb) = self
        .components
        .get(tid)
        .and_then(|vt| vt.callbacks.as_ref()?.get_create())
      {
        // i am *pretty* sure this will never be locked?
        let comp = comp.try_read().unwrap();
        cb(comp.as_ref(), e, &access);
      }
    }
  }
  fn run_removal_callback(&self, e: Entity, comps: EntityAssoc) {
    let access = CallbackWorldAccess::new(self);
    for (tid, comp) in comps.into_iter() {
      if let Some(cb) = self
        .components
        .get(&tid)
        .and_then(|vt| vt.callbacks.as_ref()?.get_remove())
      {
        let comp = comp.into_inner().unwrap();
        cb(comp, e, &access);
      }
    }
  }

  pub(crate) fn component_vt(&self, tid: TypeIdWrapper) -> &ComponentVtable {
    self.components.get(&tid).unwrap_or_else(|| {
      panic!(
        "tried to access the unregistered component type {}",
        tid.type_name
      )
    })
  }

  #[doc(hidden)]
  pub fn dump(&self) {}
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
  let msg_tid = (*msg).type_id_wrapper();

  let components = access.world.entities.get(target);
  for (comp_tid, comp) in components.iter() {
    let vt = access.world.component_vt(comp_tid);
    if let Some(handler) = vt.msg_table.get(&msg_tid) {
      let lock = comp
        .try_read()
        .unwrap_or_else(|_| loop_panic(target, comp_tid));
      let msg2 = match handler {
        MsgHandlerInner::Read(handler) => handler(&**lock, msg, target, access),
        MsgHandlerInner::Write(handler) => {
          drop(lock);
          let mut lock = comp
            .try_write()
            .unwrap_or_else(|_| loop_panic(target, comp_tid));
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
