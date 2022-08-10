//! Traits abstracting over different types of accessors and their capabilities.
//!
//! [`World`](crate::world::World) also implements these traits so you can write code
//! generic over it as well.

use crate::{
    entities::Entity,
    entities::EntityIter,
    messages::Message,
    query::Query,
    resource::{ReadResource, Resource, ResourceLookupError, WriteResource},
};

/// Trait for accesses that can dispatch messages.
pub trait AccessDispatcher {
    /// Dispatch a message to the given entity, passing it to each of its components that have registered
    /// a listener for that message type.
    fn dispatch<M: Message>(&self, target: Entity, msg: M) -> M;
}

/// Trait for accesses that can get information about entities.
pub trait AccessEntityStats {
    /// Get the number of entities in the world.
    fn len(&self) -> usize;

    /// Check if the given entity is alive in the world.
    fn is_alive(&self, entity: Entity) -> bool;

    /// Get the number of components on the given entity. Panics if the entity is dead.
    fn len_of(&self, entity: Entity) -> usize;

    /// Get an iterator over all the entities in a world.
    ///
    /// There's no way built in to Palkia to do ECS-style join queries on these entities.
    /// Just do the filtering yourself ...
    ///
    /// But, if your design prompts you to do queries over every entity, you should
    /// consider dispatching a message to every entity instead. Or just use an ECS crate.
    fn iter(&self) -> EntityIter<'_>;
}

/// Trait for accesses that can execute queries.
pub trait AccessQuery {
    /// Query the given entity for the given elements.
    ///
    /// Panics if the entity is dead.
    fn query<'c, Q: Query<'c>>(&'c self, interrogatee: Entity) -> Option<Q::Response>;
}

/// Trait for accesses that can read and write resources.
pub trait AccessResources {
    /// Get immutable access to the given resource.
    fn read_resource<R: Resource>(&self) -> Result<ReadResource<'_, R>, ResourceLookupError>;

    /// Get mutable access to the given resource.
    fn write_resource<R: Resource>(&self) -> Result<WriteResource<'_, R>, ResourceLookupError>;

    /// Check if the world contains a resource of this type.
    ///
    /// This does not require any locking, and so cannot fail.
    fn contains_resource<R: Resource>(&self) -> bool;
}
