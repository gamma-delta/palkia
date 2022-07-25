use crate::entities::EntityAssoc;
use crate::prelude::{Component, Entity, Query, World};
use crate::resource::{ReadResource, Resource, ResourceLookupError, WriteResource};

pub(crate) type OnCreateCallback =
    Box<dyn Fn(&dyn Component, Entity, &CallbackWorldAccess) + Send + Sync>;
pub(crate) type OnRemoveCallback =
    Box<dyn Fn(Box<dyn Component>, Entity, &CallbackWorldAccess) + Send + Sync>;

pub(crate) enum Callbacks {
    Create(OnCreateCallback),
    Remove(OnRemoveCallback),
    Both(OnCreateCallback, OnRemoveCallback),
}

impl Callbacks {
    pub fn get_create(&self) -> Option<&OnCreateCallback> {
        match self {
            Callbacks::Create(it) => Some(it),
            Callbacks::Remove(_) => None,
            Callbacks::Both(it, _) => Some(it),
        }
    }

    pub fn get_remove(&self) -> Option<&OnRemoveCallback> {
        match self {
            Callbacks::Create(_) => None,
            Callbacks::Remove(it) => Some(it),
            Callbacks::Both(_, it) => Some(it),
        }
    }
}

/// Access you have to the world during a callback.
///
/// You can do most things a `ListenerWorldAccess` lets you do, but you can't spawn/delete
/// new entities or dispatch events. You should mostly be using this to update resources,
/// like if you have a cache of location->entities.
pub struct CallbackWorldAccess<'w> {
    world: &'w World,
}

impl<'w> CallbackWorldAccess<'w> {
    pub(crate) fn new(world: &'w World) -> Self {
        Self { world }
    }

    /// Get immutable access to the given resource.
    pub fn read_resource<R: Resource>(&self) -> Result<ReadResource<'_, R>, ResourceLookupError> {
        self.world.resources.read()
    }

    /// Get mutable access to the given resource.
    pub fn write_resource<R: Resource>(&self) -> Result<WriteResource<'_, R>, ResourceLookupError> {
        self.world.resources.write()
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
}
