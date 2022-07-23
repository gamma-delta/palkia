use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Debug;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::RwLock;

use crossbeam::channel;

use crate::builder::{ImmediateEntityBuilder, LazyEntityBuilder};
use crate::component::{Component, ListenerBuilder};
use crate::entities::{Entity, EntityAllocator, EntityAssoc};
use crate::events::{Event, EventListener};
use crate::prelude::Query;
use crate::resource::{ReadResource, Resource, ResourceLookupError, ResourceMap, WriteResource};
use crate::{loop_panic, ToTypeIdWrapper, TypeIdWrapper};

pub struct World {
    /// Each entity maps type IDs to their components
    pub(crate) entities: EntityAllocator<EntityAssoc>,
    /// Maps event types to, maps component types to the EventHandler.
    event_table: BTreeMap<TypeIdWrapper, BTreeMap<TypeIdWrapper, EventListener>>,
    pub(crate) known_component_types: BTreeSet<TypeIdWrapper>,

    resources: ResourceMap,

    lazy_sender: channel::Sender<LazyUpdate>,
    lazy_channel: channel::Receiver<LazyUpdate>,
    lazy_entities_created: AtomicUsize,
    lazy_entities_deleted: AtomicU64,
}

impl World {
    pub fn new() -> World {
        let (tx, rx) = channel::unbounded();

        Self {
            entities: EntityAllocator::new(),
            event_table: BTreeMap::new(),
            resources: ResourceMap::new(),
            known_component_types: BTreeSet::new(),
            lazy_sender: tx,
            lazy_channel: rx,
            lazy_entities_created: AtomicUsize::new(0),
            lazy_entities_deleted: AtomicU64::new(0),
        }
    }

    pub fn register_component<C: Component>(&mut self) {
        self.known_component_types.insert(TypeIdWrapper::of::<C>());

        let builder = ListenerBuilder::<C>::new();
        let builder = C::register_listeners(builder);
        for (ev_type, handler) in builder.listeners {
            self.event_table
                .entry(ev_type)
                .or_default()
                .insert(TypeIdWrapper::of::<C>(), handler);
        }
    }

    /// Set up a builder to spawn an entity with a whole mess of components.
    pub fn spawn(&mut self) -> ImmediateEntityBuilder<'_> {
        let to_create = self.spawn_empty();
        ImmediateEntityBuilder::new(self, to_create)
    }

    /// Spawn a new empty entity.
    pub fn spawn_empty(&mut self) -> Entity {
        self.entities.insert(Vec::new())
    }

    /// As a convenience method, spawn an entity with a single component.
    pub fn spawn_1<C: Component>(&mut self, component: C) -> Entity {
        let comps = vec![RwLock::new(Box::new(component) as _)];
        self.entities.insert(comps)
    }

    /// Dispatch an event to the given entity.
    ///
    /// Return the modified event.
    pub fn dispatch<E: Event>(&self, target: Entity, event: E) -> E {
        dispatch_inner(&WorldAccess::new(self), target, event)
    }

    /// Dispatch an event to all entities, cloning it for each entity.
    pub fn dispatch_to_all<E: Event + Clone>(&self, event: E) {
        let entities = self.entities.iter().map(|(e, _)| e).collect::<Vec<_>>();
        for e in entities {
            self.dispatch(e, event.clone());
        }
    }

    /// Query the given entity for the given elements. If the entity is dead, returns `None`.
    pub fn query<'c, Q: Query<'c>>(&'c self, interrogatee: Entity) -> Option<Q::Response> {
        let comps = self.entities.get(interrogatee)?;
        Q::query(interrogatee, comps)
    }

    /// Check if the given entity is, at this moment, still alive.
    pub fn is_alive(&self, e: Entity) -> bool {
        self.entities.get(e).is_some()
    }

    /// Get the number of components on the given entity, or `None` if it's dead.
    pub fn len_of(&self, e: Entity) -> Option<usize> {
        self.entities.get(e).map(Vec::len)
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

    /// Get immutable access to the given resource.
    pub fn read_resource<R: Resource>(&self) -> Result<ReadResource<'_, R>, ResourceLookupError> {
        self.resources.read()
    }

    /// Get mutable access to the given resource.
    pub fn write_resource<R: Resource>(&self) -> Result<WriteResource<'_, R>, ResourceLookupError> {
        self.resources.write()
    }

    /// With ownership, get direct mutable access to the given resource.
    pub fn get_resource<R: Resource>(&mut self) -> Option<&mut R> {
        self.resources.get()
    }

    /// Get the number of entities in the world.
    pub fn len(&self) -> usize {
        self.entities.len()
    }

    /// Apply any and all lazy updates.
    pub fn finalize(&mut self) {
        let updates = self.lazy_channel.try_iter().collect::<Vec<_>>();
        for lazy in updates {
            lazy.apply(self);
        }
        *self.lazy_entities_created.get_mut() = 0;
        *self.lazy_entities_deleted.get_mut() = 0;
    }
}

/// Way to access a world from an event listener.
///
/// Some of the changes here won't actually apply until `World::finalize` is called.
pub struct WorldAccess<'w> {
    lazy_updates: channel::Sender<LazyUpdate>,

    world: &'w World,
}

impl<'w> WorldAccess<'w> {
    pub(crate) fn new(world: &'w World) -> Self {
        Self {
            lazy_updates: world.lazy_sender.clone(),
            world,
        }
    }

    /// Get immutable access to the given resource.
    pub fn read_resource<R: Resource>(&self) -> Result<ReadResource<'_, R>, ResourceLookupError> {
        self.world.resources.read()
    }

    /// Get mutable access to the given resource.
    pub fn write_resource<R: Resource>(&self) -> Result<WriteResource<'_, R>, ResourceLookupError> {
        self.world.resources.write()
    }

    /// Dispatch an event to the given entity.
    pub fn dispatch<E: Event>(&self, target: Entity, event: E) -> E {
        dispatch_inner(self, target, event)
    }

    /// Queue an entity to be spawned when [`World::finalize`] is called.
    pub fn lazy_spawn<'a>(&'a self) -> LazyEntityBuilder<'a, 'w> {
        let entities_spawned = self
            .world
            .lazy_entities_created
            .fetch_add(1, Ordering::SeqCst);
        let entity = Entity {
            generation: self.world.entities.generation()
                + self.world.lazy_entities_deleted.load(Ordering::SeqCst),
            index: self.world.entities.capacity() + entities_spawned,
        };
        LazyEntityBuilder::new(self, entity)
    }

    /// Queue an entity to be despawned when [`World::finalize`] is called.
    pub fn lazy_despawn(&self, entity: Entity) {
        self.world
            .lazy_entities_deleted
            .fetch_add(1, Ordering::SeqCst);
        self.queue_update(LazyUpdate::DespawnEntity(entity));
    }

    /// Query the given entity for the given elements. If the entity is dead, returns `None`.
    pub fn query<'c, Q: Query<'c>>(&'c self, interrogatee: Entity) -> Option<Q::Response> {
        let comps = self.world.entities.get(interrogatee)?;
        Q::query(interrogatee, comps)
    }

    /// Check if the given entity is, at this moment, still alive.
    pub fn is_alive(&self, e: Entity) -> bool {
        self.world.entities.get(e).is_some()
    }

    /// Get the number of components on the given entity, or `None` if it's dead.
    pub fn len_of(&self, e: Entity) -> Option<usize> {
        self.world.entities.get(e).map(Vec::len)
    }

    pub(crate) fn queue_update(&self, update: LazyUpdate) {
        self.lazy_updates.send(update).unwrap();
    }
}

pub(crate) enum LazyUpdate {
    SpawnEntity(Vec<Box<dyn Component>>, Entity),
    DespawnEntity(Entity),
}

impl LazyUpdate {
    fn apply(self, world: &mut World) {
        match self {
            LazyUpdate::SpawnEntity(comps, expect) => {
                let out = comps
                    .into_iter()
                    .map(|comp| {
                        if !world
                            .known_component_types
                            .contains(&(*comp).type_id_wrapper())
                        {
                            panic!(
                                "tried to insert a component with unregistered type {}",
                                (*comp).type_name()
                            )
                        }
                        RwLock::new(comp)
                    })
                    .collect();
                let new = world.entities.insert_increasing(out);
                debug_assert_eq!(new, expect);
            }
            LazyUpdate::DespawnEntity(entity) => {
                world.entities.remove(entity);
            }
        }
    }
}

impl Debug for LazyUpdate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SpawnEntity(arg0, arg1) => f
                .debug_tuple("SpawnEntity")
                .field(&format_args!("Vec(len {})", arg0.len()))
                .field(arg1)
                .finish(),
            Self::DespawnEntity(arg0) => f.debug_tuple("DespawnEntity").field(arg0).finish(),
        }
    }
}

fn dispatch_inner<E: Event>(access: &WorldAccess, target: Entity, mut event: E) -> E {
    let event_handlers = match access.world.event_table.get(&TypeIdWrapper::of::<E>()) {
        Some(it) => it,
        None => {
            // i've never met this event type in my life
            return event;
        }
    };

    let components = access.world.entities.get(target).unwrap();
    for comp in components.iter() {
        let lock = comp.try_read().unwrap_or_else(|_| loop_panic(target));
        let tid = (**lock).type_id_wrapper();
        if let Some(handler) = event_handlers.get(&tid) {
            let event2 = match handler {
                EventListener::Read(handler) => handler(&**lock, Box::new(event), target, access),
                EventListener::Write(handler) => {
                    drop(lock);
                    let mut lock = comp.try_write().unwrap_or_else(|_| loop_panic(target));
                    handler(&mut **lock, Box::new(event), target, access)
                }
            };
            // SAFETY: the type ID guards prevent these from being different types.
            event = unsafe { *event2.downcast().unwrap_unchecked() };
        }
    }

    event
}
