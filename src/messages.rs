use std::sync::atomic::Ordering;

use crossbeam::channel;
use downcast::{downcast, AnySync};

use crate::builder::LazyEntityBuilder;
use crate::entities::EntityAssoc;
use crate::prelude::{Component, Entity, Query, World};
use crate::resource::{ReadResource, Resource, ResourceLookupError, WriteResource};
use crate::world::{dispatch_inner, LazyUpdate};

/// Data that is threaded through components.
///
/// When a message is dispatched to an entity, it goes through its components. A component with a handler for this type
/// registered with [`World::register_component`] gets its listener called, and then the updated event gets passed to the next
/// component ... and so on. Then, it's returned to the dispatcher.
pub trait Message: AnySync {}
downcast!(dyn Message);

/// A message handler that only needs immutable access to the component.
pub type MsgHandlerRead<C, E> =
    fn(this: &C, event: E, owner: Entity, access: &ListenerWorldAccess) -> E;
/// A message handler that needs mutable access to the component.
pub type MsgHandlerWrite<C, E> =
    fn(this: &mut C, event: E, owner: Entity, access: &ListenerWorldAccess) -> E;

pub(crate) enum MsgHandlerInner {
    Read(
        Box<
            dyn Send
                + Sync
                + Fn(
                    &dyn Component,
                    Box<dyn Message>,
                    Entity,
                    &ListenerWorldAccess,
                ) -> Box<dyn Message>,
        >,
    ),
    Write(
        Box<
            dyn Send
                + Sync
                + Fn(
                    &mut dyn Component,
                    Box<dyn Message>,
                    Entity,
                    &ListenerWorldAccess,
                ) -> Box<dyn Message>,
        >,
    ),
}

/// Way to access a world from a message listener.
///
/// Some of the changes here won't actually apply until `World::finalize` is called.
pub struct ListenerWorldAccess<'w> {
    lazy_updates: channel::Sender<LazyUpdate>,

    pub(crate) world: &'w World,
}

impl<'w> ListenerWorldAccess<'w> {
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

    /// Dispatch a message to the given entity.
    pub fn dispatch<M: Message>(&self, target: Entity, msg: M) -> M {
        dispatch_inner(self, target, msg)
    }

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
        self.world.entities.get(e).map(EntityAssoc::len)
    }

    pub(crate) fn queue_update(&self, update: LazyUpdate) {
        self.lazy_updates.send(update).unwrap();
    }
}
