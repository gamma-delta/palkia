//! Lazy and unlazy entity builders.
//!
//! See [`EntityBuilder`] for the generic interface, or [`ImmediateEntityBuilder`] and [`LazyEntityBuilder`]
//! for concrete impls.

// should this go in the access module?

use std::collections::{BTreeMap, BTreeSet};

use crate::prelude::{Component, Entity, ListenerWorldAccess, World};
use crate::world::LazyUpdate;
use crate::TypeIdWrapper;
use crate::{entities::EntityAssoc, ToTypeIdWrapper};

/// Unified interface for [`ImmediateEntityBuilder`] and [`LazyEntityBuilder`], for ease of generic code.
///
/// When sending a message to an entity, components will recieve the message in the order that things were
/// added to the builder.
pub unsafe trait EntityBuilder: Sized {
    /// Insert the given type-erased component into the entity.
    /// If there was a component with that type already on the entity, replaces and returns the old component.
    ///
    /// You should probably not be calling this; try [`insert`][EntityBuilder::insert].
    ///
    /// SAFETY: This must *only ever* return the *same* type of component as passed in if it returns anything.
    fn insert_raw(&mut self, component: Box<dyn Component>) -> Option<Box<dyn Component>>;

    /// Insert the given component into the tentative entity. If there was a component with that type already on
    /// the entity, replaces and returns the old component.
    fn insert<C: Component>(&mut self, component: C) -> Option<C> {
        let erased = Box::new(component);
        self.insert_raw(erased).map(|cmp| {
            // SAFETY: the unsafe impl of `insert_erased` must be implemented correctly to not return a bad type
            unsafe { *cmp.downcast().unwrap_unchecked() }
        })
    }

    /// Insert the given component into the entity. Like [`insert`][`EntityBuilder::insert`], but returns `self`
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
    ///
    /// Note that if you *don't* call this, there will be panics.
    fn build(self) -> Entity;
}

#[derive(Default)]
pub(crate) struct EntityBuilderComponentTracker {
    pub(crate) components: Vec<Box<dyn Component>>,
    component_ids: BTreeMap<TypeIdWrapper, usize>,
}

impl EntityBuilderComponentTracker {
    pub(crate) fn insert<C: Component>(
        &mut self,
        component: C,
        comp_types: &BTreeSet<TypeIdWrapper>,
    ) -> Option<C> {
        self.insert_raw(Box::new(component), comp_types)
            .map(|comp| {
                // SAFETY: type guards
                unsafe { *comp.downcast().unwrap_unchecked() }
            })
    }

    pub(crate) fn insert_raw(
        &mut self,
        component: Box<dyn Component>,
        comp_types: &BTreeSet<TypeIdWrapper>,
    ) -> Option<Box<dyn Component>> {
        let tid = (*component).type_id_wrapper();
        if !comp_types.contains(&tid) {
            // Technically, no UB or anything happens if this doesn't panic, but it *is* an easy mistake to make
            // and your events won't fire.
            panic!("tried to add a component of type {} to an entity, but that type was not registered", tid.type_name);
        }

        if let Some(clobberee) = self.component_ids.get(&tid) {
            let old = std::mem::replace(&mut self.components[*clobberee], component);
            Some(old)
        } else {
            let idx = self.components.len();
            self.components.push(component);
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
    tracker: EntityBuilderComponentTracker,
}

impl<'w> ImmediateEntityBuilder<'w> {
    pub(crate) fn new(world: &'w mut World, entity: Entity) -> Self {
        Self {
            world,
            entity,
            tracker: EntityBuilderComponentTracker::default(),
        }
    }
}

unsafe impl<'w> EntityBuilder for ImmediateEntityBuilder<'w> {
    fn insert_raw(&mut self, component: Box<dyn Component>) -> Option<Box<dyn Component>> {
        self.tracker
            .insert_raw(component, &self.world.known_component_types)
    }

    fn len(&self) -> usize {
        self.tracker.components.len()
    }

    fn build(self) -> Entity {
        self.world
            .entities
            .finish_spawn(self.entity, EntityAssoc::new(self.tracker.components));
        self.world.run_creation_callbacks(self.entity);
        self.entity
    }
}

/// An [`EntityBuilder`] that does not have a mutable reference to the world.
///
/// The entity will be *queued* to be inserted when `.build()` is called, but won't actually
/// exist until whatever event handler it's being called from returns.
#[must_use = "Does nothing until `.build()` is called."]
pub struct LazyEntityBuilder<'a, 'w> {
    accessor: &'a ListenerWorldAccess<'w>,
    pub entity: Entity,
    tracker: EntityBuilderComponentTracker,
}

impl<'a, 'w> LazyEntityBuilder<'a, 'w> {
    pub(crate) fn new(accessor: &'a ListenerWorldAccess<'w>, entity: Entity) -> Self {
        Self {
            accessor,
            entity,
            tracker: EntityBuilderComponentTracker::default(),
        }
    }
}

unsafe impl<'a, 'w> EntityBuilder for LazyEntityBuilder<'a, 'w> {
    fn insert_raw(&mut self, component: Box<dyn Component>) -> Option<Box<dyn Component>> {
        self.tracker
            .insert_raw(component, &self.accessor.world.known_component_types)
    }

    fn len(&self) -> usize {
        self.tracker.components.len()
    }

    fn build(self) -> Entity {
        self.accessor.queue_update(LazyUpdate::FinishEntity(
            self.tracker.components,
            self.entity,
        ));
        self.entity
    }
}
