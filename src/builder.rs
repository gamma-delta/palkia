use std::collections::BTreeMap;
use std::sync::RwLock;

use crate::entities::EntityAssoc;
use crate::prelude::{Component, Entity, World};
use crate::world::{LazyUpdate, WorldAccess};
use crate::{ToTypeIdWrapper, TypeIdWrapper};

/// Unified interface for [`ImmediateEntityBuilder`] and [`LazyEntityBuilder`], for ease of generic code.
pub trait EntityBuilder: Sized {
    /// Insert the given component into the entity, returning the old value of the component
    /// that was there if any.
    fn insert<C: Component>(&mut self, component: C) -> Option<C>;

    /// Insert the given component into the entity. Like [`Self::insert`], but returns `self`
    /// for chaining.
    fn with<C: Component>(mut self, component: C) -> Self {
        self.insert(component);
        self
    }

    /// Get the number of components that will be attached to the given entity.
    fn len(&self) -> usize;

    /// Return true if no components will be attached to this entity.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Consume this and insert the entity into the world, returning it to the caller.
    fn build(self) -> Entity;
}

#[derive(Default)]
struct EntityBuilderComponentTracker {
    components: Vec<Box<dyn Component>>,
    component_ids: BTreeMap<TypeIdWrapper, usize>,
}

impl EntityBuilderComponentTracker {
    fn insert<C: Component>(&mut self, component: C) -> Option<C> {
        let tid = TypeIdWrapper::of::<C>();
        let boxc = Box::new(component) as _;
        if let Some(clobberee) = self.component_ids.get(&tid) {
            let old = std::mem::replace(&mut self.components[*clobberee], boxc);
            Some(*old.downcast().unwrap())
        } else {
            let idx = self.components.len();
            self.components.push(boxc);
            self.component_ids.insert(tid, idx);
            None
        }
    }
}

/// An [`EntityBuilder`] made with exclusive, mutable access to the world.
///
/// The entity is inserted as soon as `.build()` is called.
#[must_use = "Does nothing until `.build()` is called."]
pub struct ImmediateEntityBuilder<'w> {
    world: &'w mut World,
    pub entity: Entity,
    assoc: EntityAssoc,
}

impl<'w> ImmediateEntityBuilder<'w> {
    pub(crate) fn new(world: &'w mut World, entity: Entity) -> Self {
        Self {
            world,
            entity,
            assoc: EntityAssoc::new(),
        }
    }
}

impl<'w> EntityBuilder for ImmediateEntityBuilder<'w> {
    fn insert<C: Component>(&mut self, component: C) -> Option<C> {
        self.assoc.insert(component)
    }

    fn len(&self) -> usize {
        self.assoc.components().len()
    }

    fn build(self) -> Entity {
        let here = self.world.entities.get_mut(self.entity).unwrap();
        debug_assert_eq!(here.components().len(), 0); // doing this instead of is_empty so if it fails I can see the len
        *here = self.assoc;
        self.entity
    }
}

/// An [`EntityBuilder`] that does not have a mutable reference to the world.
///
/// The entity will be *queued* to be inserted when `.build()` is called, but won't actually
/// exist until whatever event handler it's being called from returns.
#[must_use = "Does nothing until `.build()` is called."]
pub struct LazyEntityBuilder<'a, 'w> {
    accessor: &'a WorldAccess<'w>,
    pub entity: Entity,
    assoc: EntityAssoc,
}

impl<'a, 'w> LazyEntityBuilder<'a, 'w> {
    pub(crate) fn new(accessor: &'a WorldAccess<'w>, entity: Entity) -> Self {
        Self {
            accessor,
            entity,
            assoc: EntityAssoc::new(),
        }
    }
}

impl<'a, 'w> EntityBuilder for LazyEntityBuilder<'a, 'w> {
    fn insert<C: Component>(&mut self, component: C) -> Option<C> {
        self.assoc.insert(component)
    }

    fn len(&self) -> usize {
        self.assoc.components().len()
    }

    fn build(self) -> Entity {
        self.accessor
            .queue_update(LazyUpdate::SpawnEntity(self.assoc, self.entity));
        self.entity
    }
}
