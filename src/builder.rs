//! Lazy and unlazy entity builders.
//!
//! See [`EntityBuilder`] for the generic interface, or [`ImmediateEntityBuilder`] and [`LazyEntityBuilder`]
//! for concrete impls.

// should this go in the access module?

use std::collections::{BTreeMap};

use crate::{
  prelude::{Component, Entity, ListenerWorldAccess, World},
  world::{EntityAssoc, LazyUpdate},
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

  pub(crate) fn new_lazy_world(world: &'w World, entity: Entity) -> Self {
    Self {
      entity,
      tracker: EntityBuilderComponentTracker::new(),
      access: EntityBuilderAccess::LazyWorld(world),
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
    let _world = match self.access {
      EntityBuilderAccess::Immediate(ref world) => world,
      EntityBuilderAccess::Lazy(lazy) => lazy.world,
      EntityBuilderAccess::LazyWorld(ref world) => world,
    };
    self.tracker.insert_raw(component)
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
        world
          .finish_spawn(self.entity, EntityAssoc::new(self.tracker.components));
      }
      EntityBuilderAccess::Lazy(lazy) => {
        lazy.queue_update(LazyUpdate::FinishEntity(
          self.tracker.components,
          self.entity,
        ));
      }
      EntityBuilderAccess::LazyWorld(world) => {
        world
          .lazy_sender
          .send(LazyUpdate::FinishEntity(
            self.tracker.components,
            self.entity,
          ))
          .unwrap();
      }
    }
    self.entity
  }

  /// Get raw access to the builder's view on the world.
  pub fn get_access(&self) -> &EntityBuilderAccess<'a, 'w> {
    &self.access
  }

  /// Get raw mutable access to the builder's view on the world.
  pub fn get_access_mut(&mut self) -> &mut EntityBuilderAccess<'a, 'w> {
    &mut self.access
  }

  /// If a component with the given type exists on the builder,
  /// get a reference to it.
  pub fn get_component<C: Component>(&self) -> Option<&C> {
    let tid = TypeIdWrapper::of::<C>();
    let idx = self.tracker.component_idxs.get(&tid)?;
    let boxed = &self.tracker.components[*idx];
    // SAFETY: type guards
    Some(unsafe { boxed.downcast_ref().unwrap_unchecked() })
  }

  /// If a component with the given type exists on the builder,
  /// get a mutable referece to it.
  pub fn get_component_mut<C: Component>(&mut self) -> Option<&mut C> {
    let tid = TypeIdWrapper::of::<C>();
    let idx = self.tracker.component_idxs.get(&tid)?;
    let boxed = &mut self.tracker.components[*idx];
    // SAFETY: type guards
    Some(unsafe { boxed.downcast_mut().unwrap_unchecked() })
  }

  /// Create a new [`EntityBuilder`] from this one. It will be lazy or unlazy
  /// the same as `self` is.
  ///
  /// Mostly for use in Dialga.
  pub fn spawn_again<'me, 'a2, 'w2>(&'me mut self) -> EntityBuilder<'a2, 'w2>
  where
    'a: 'a2,
    'w: 'w2,
    'me: 'a2,
    'me: 'w2,
  {
    match self.access {
      EntityBuilderAccess::Immediate(ref mut world) => world.spawn(),
      EntityBuilderAccess::Lazy(lazy) => lazy.lazy_spawn(),
      EntityBuilderAccess::LazyWorld(world) => world.lazy_spawn(),
    }
  }
}

/// Access that an EntityBuilder gets to the world, whether immediate or deferred.
pub enum EntityBuilderAccess<'a, 'w> {
  Immediate(&'w mut World),
  Lazy(&'a ListenerWorldAccess<'w>),
  LazyWorld(&'a World),
}

#[derive(Default)]
pub(crate) struct EntityBuilderComponentTracker {
  pub(crate) components: Vec<Box<dyn Component>>,
  component_idxs: BTreeMap<TypeIdWrapper, usize>,
}

impl EntityBuilderComponentTracker {
  pub(crate) fn new() -> Self {
    Self::default()
  }

  pub(crate) fn insert<C: Component>(&mut self, component: C) -> Option<C> {
    self.insert_raw(Box::new(component)).map(|comp| {
      // SAFETY: type guards
      unsafe { *comp.downcast().unwrap_unchecked() }
    })
  }

  pub(crate) fn insert_raw(
    &mut self,
    component: Box<dyn Component>,
  ) -> Option<Box<dyn Component>> {
    let tid = (*component).type_id_wrapper();
    if let Some(clobberee) = self.component_idxs.get(&tid) {
      let old = std::mem::replace(&mut self.components[*clobberee], component);
      Some(old)
    } else {
      let idx = self.components.len();
      self.components.push(component);
      self.component_idxs.insert(tid, idx);
      None
    }
  }
}
