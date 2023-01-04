//! Lazy and unlazy entity builders.
//!
//! See [`EntityBuilder`] for the generic interface, or [`ImmediateEntityBuilder`] and [`LazyEntityBuilder`]
//! for concrete impls.

// should this go in the access module?

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    entities::EntityAssoc,
    prelude::{Component, Entity, ListenerWorldAccess, World},
    world::LazyUpdate,
    ToTypeIdWrapper, TypeIdWrapper,
};

/// When sending a message to an entity, components will recieve the message in
/// the order that things were added to the builder.
///
/// This struct has two variants internally;
/// an immediate mode for when you have mutable access to the builder
/// and a lazy mode that adds the entity only once [`World::finalize`] is
/// called.
#[must_use = "Does nothing until `.build()` is called."]
pub struct EntityBuilder<'a, 'w> {
    pub entity: Entity,
    tracker: EntityBuilderComponentTracker,
    access: EntityBuilderAccess<'a, 'w>,
}

impl<'a, 'w> EntityBuilder<'a, 'w> {
    pub(crate) fn new_lazy(
        lazy: &'a ListenerWorldAccess<'w>,
        entity: Entity,
    ) -> Self {
        Self {
            entity,
            tracker: EntityBuilderComponentTracker::new(),
            access: EntityBuilderAccess::Lazy(lazy),
        }
    }

    pub(crate) fn new_immediate(world: &'w mut World, entity: Entity) -> Self {
        Self {
            entity,
            tracker: EntityBuilderComponentTracker::new(),
            access: EntityBuilderAccess::Immediate(world),
        }
    }

    /// Insert the given type-erased component into the entity.
    /// If there was a component with that type already on the entity,
    /// replaces and returns the old component.
    ///
    /// You should probably not be calling this; try [`insert`][EntityBuilder::insert].
    pub fn insert_raw(
        &mut self,
        component: Box<dyn Component>,
    ) -> Option<Box<dyn Component>> {
        let world = match self.access {
            EntityBuilderAccess::Immediate(ref world) => world,
            EntityBuilderAccess::Lazy(lazy) => lazy.world,
        };
        self.tracker
            .insert_raw(component, &world.known_component_types)
    }

    /// Insert the given component into the tentative entity.
    /// If there was a component with that type already on the entity,
    /// replaces and returns the old component.
    pub fn insert<C: Component>(&mut self, component: C) -> Option<C> {
        let erased = Box::new(component);
        self.insert_raw(erased).map(|cmp| {
            // SAFETY: type id guard
            unsafe { *cmp.downcast().unwrap_unchecked() }
        })
    }

    /// Insert the given component into the entity.
    /// Like [`insert`][`EntityBuilder::insert`], but returns `self`
    /// for chaining.
    pub fn with<C: Component>(mut self, component: C) -> Self
    where
        Self: Sized,
    {
        self.insert(component);
        self
    }

    /// Get the number of components that will be attached to the given entity.
    pub fn len(&self) -> usize {
        self.tracker.components.len()
    }

    /// Return true if no components will be attached to this entity.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Consume this and insert the entity into the world, returning it to the caller.
    ///
    /// Note that if you *don't* call this, there will be panics.
    pub fn build(mut self) -> Entity {
        match self.access {
            EntityBuilderAccess::Immediate(ref mut world) => {
                world.entities.finish_spawn(
                    self.entity,
                    EntityAssoc::new(self.tracker.components),
                );
                world.run_creation_callbacks(self.entity);
            }
            EntityBuilderAccess::Lazy(lazy) => {
                lazy.queue_update(LazyUpdate::FinishEntity(
                    self.tracker.components,
                    self.entity,
                ));
            }
        }
        self.entity
    }

    /// Get whether this is immediate mode or not, if you care for some reason.
    pub fn is_immediate(&self) -> bool {
        match self.access {
            EntityBuilderAccess::Immediate(_) => true,
            EntityBuilderAccess::Lazy(_) => false,
        }
    }
}

pub(crate) enum EntityBuilderAccess<'a, 'w> {
    Immediate(&'w mut World),
    Lazy(&'a ListenerWorldAccess<'w>),
}

#[derive(Default)]
pub(crate) struct EntityBuilderComponentTracker {
    pub(crate) components: Vec<Box<dyn Component>>,
    component_ids: BTreeMap<TypeIdWrapper, usize>,
}

impl EntityBuilderComponentTracker {
    pub(crate) fn new() -> Self {
        Self::default()
    }

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
            let old =
                std::mem::replace(&mut self.components[*clobberee], component);
            Some(old)
        } else {
            let idx = self.components.len();
            self.components.push(component);
            self.component_ids.insert(tid, idx);
            None
        }
    }
}
