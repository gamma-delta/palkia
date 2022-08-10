//! Lazy and unlazy entity builders.
//!
//! See [`EntityBuilder`] for the generic interface, or [`ImmediateEntityBuilder`] and [`LazyEntityBuilder`]
//! for concrete impls.

// should this go in the access module?

use std::collections::{BTreeMap, BTreeSet};

use crate::entities::EntityAssoc;
use crate::prelude::{Component, Entity, ListenerWorldAccess, World};
use crate::world::LazyUpdate;
use crate::TypeIdWrapper;

/// Unified interface for [`ImmediateEntityBuilder`] and [`LazyEntityBuilder`], for ease of generic code.
///
/// When sending a message to an entity, components will recieve the message in the order that things were
/// added to the builder.
pub trait EntityBuilder: Sized {
    /// Insert the given component into the entity. If there was a component with that type already on
    /// the entity, replaces and returns the old component.
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
    ///
    /// Note that if you *don't* call this, the entity will be leaked. An entity will still be
    /// created, but nothing will be put on it.
    fn build(self) -> Entity;
}

#[derive(Default)]
struct EntityBuilderComponentTracker {
    components: Vec<Box<dyn Component>>,
    component_ids: BTreeMap<TypeIdWrapper, usize>,
}

impl EntityBuilderComponentTracker {
    fn insert<C: Component>(
        &mut self,
        component: C,
        comp_types: &BTreeSet<TypeIdWrapper>,
    ) -> Option<C> {
        let tid = TypeIdWrapper::of::<C>();
        if !comp_types.contains(&tid) {
            // Technically, no UB or anything happens if this doesn't panic, but it *is* an easy mistake to make
            // and your events won't fire.
            panic!("tried to add a component of type {} to an entity, but that type was not registered", tid.type_name);
        }

        let boxc = Box::new(component) as _;
        if let Some(clobberee) = self.component_ids.get(&tid) {
            let old = std::mem::replace(&mut self.components[*clobberee], boxc);
            // SAFETY: type guards prevent this from being of the wrong type
            Some(unsafe { *old.downcast().unwrap_unchecked() })
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

impl<'w> EntityBuilder for ImmediateEntityBuilder<'w> {
    fn insert<C: Component>(&mut self, component: C) -> Option<C> {
        self.tracker
            .insert(component, &self.world.known_component_types)
    }

    fn len(&self) -> usize {
        self.tracker.components.len()
    }

    fn build(self) -> Entity {
        let here = self.world.entities.get_mut(self.entity).unwrap();
        debug_assert_eq!(here.components().len(), 0); // doing this instead of is_empty so if it fails I can see the len
        *here = EntityAssoc::new(self.tracker.components);

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

impl<'a, 'w> EntityBuilder for LazyEntityBuilder<'a, 'w> {
    fn insert<C: Component>(&mut self, component: C) -> Option<C> {
        self.tracker
            .insert(component, &self.accessor.world.known_component_types)
    }

    fn len(&self) -> usize {
        self.tracker.components.len()
    }

    fn build(self) -> Entity {
        self.accessor.queue_update(LazyUpdate::SpawnEntity(
            self.tracker.components,
            self.entity,
        ));
        self.entity
    }
}
