//! Run code when spawning or despawning an entity with a given component type.

use crate::{
  access::{AccessEntityStats, AccessQuery, AccessResources},
  entities::EntityLiveness,
  prelude::{Component, Entity, Query, World},
  resource::{ReadResource, Resource, ResourceLookupError, WriteResource},
};

pub(crate) type OnCreateCallback =
  Box<dyn Fn(&dyn Component, Entity, &CallbackWorldAccess) + Send + Sync>;
pub(crate) type OnRemoveCallback =
  Box<dyn Fn(&dyn Component, Entity, &CallbackWorldAccess) + Send + Sync>;

/// Access you have to the world during a callback.
///
/// You should mostly be using this to update resources, like if you have a
/// cache of location->entities.
pub struct CallbackWorldAccess<'w> {
  world: &'w World,
}

impl<'w> CallbackWorldAccess<'w> {
  pub(crate) fn new(world: &'w World) -> Self {
    Self { world }
  }

  /// Get immutable access to the given resource.
  pub fn read_resource<R: Resource>(
    &self,
  ) -> Result<ReadResource<'_, R>, ResourceLookupError> {
    self.world.resources.read()
  }

  /// Get mutable access to the given resource.
  pub fn write_resource<R: Resource>(
    &self,
  ) -> Result<WriteResource<'_, R>, ResourceLookupError> {
    self.world.resources.write()
  }
}

impl<'w> AccessEntityStats for CallbackWorldAccess<'w> {
  fn len(&self) -> usize {
    self.world.len()
  }

  fn liveness(&self, entity: Entity) -> EntityLiveness {
    self.world.liveness(entity)
  }

  fn len_of(&self, entity: Entity) -> usize {
    self.world.len_of(entity)
  }

  fn iter(&self) -> crate::entities::EntityIter<'_> {
    self.world.iter()
  }
}

impl<'w> AccessQuery for CallbackWorldAccess<'w> {
  fn query<'c, Q: Query<'c>>(
    &'c self,
    interrogatee: Entity,
  ) -> Option<Q::Response> {
    self.world.query::<Q>(interrogatee)
  }
}

impl<'w> AccessResources for CallbackWorldAccess<'w> {
  fn read_resource<R: Resource>(
    &self,
  ) -> Result<ReadResource<'_, R>, ResourceLookupError> {
    self.world.read_resource()
  }

  fn write_resource<R: Resource>(
    &self,
  ) -> Result<WriteResource<'_, R>, ResourceLookupError> {
    self.world.write_resource()
  }

  fn contains_resource<R: Resource>(&self) -> bool {
    self.world.contains_resource::<R>()
  }
}
