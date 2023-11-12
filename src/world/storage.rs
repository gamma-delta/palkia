use std::sync::RwLock;

use ahash::AHashMap;
use generational_arena::Arena;
use indexmap::IndexMap;

use crate::{
  entities::EntityIter,
  prelude::{Component, Entity, EntityLiveness},
  ToTypeIdWrapper, TypeIdWrapper,
};

/// Allocator and storage for entities.
///
/// This creates indices with an allocator protected by a lock, and maps them
/// to the actual (unlocked) data bundles in a separate map. This way we can
/// get accurate indices for lazily created entities and less performance
/// overhead than locking and unlocking for the assocs all the time.
///
/// An entity present in the allocator but not the assocs means it's only
/// been lazily created.
#[derive(Default)]
pub(crate) struct EntityStorage {
  /// This is only public for serde
  pub allocator: RwLock<Arena<()>>,
  assocs: AHashMap<Entity, EntityAssoc>,
}

impl EntityStorage {
  pub(crate) fn new(
    allocator: Arena<()>,
    assocs: AHashMap<Entity, EntityAssoc>,
  ) -> Self {
    Self {
      allocator: RwLock::new(allocator),
      assocs,
    }
  }

  /// Lazily spawn an entity. This creates a slot for it, but does not put any
  /// data in it.
  pub fn spawn_unfinished(&self) -> Entity {
    let mut lock = self.allocator.try_write().unwrap();
    Entity(lock.insert(()))
  }

  pub fn finish_spawn(&mut self, target: Entity, assoc: EntityAssoc) {
    match self.assocs.insert(target, assoc) {
      None => {} // all good
      Some(..) => {
        panic!("tried to finish spawning an entity that was already alive")
      }
    }
  }

  /// Immediately despawn the given entity.
  ///
  /// Returns the associated data in case you want it for some reason
  pub fn despawn(&mut self, target: Entity) -> EntityAssoc {
    let alloc = self.allocator.get_mut().unwrap();
    if alloc.remove(target.0).is_none() {
      panic!("tried to despawn an entity that was not in the allocator");
    }

    let assoc = self.assocs.remove(&target);
    match assoc {
      Some(it) => it,
      None => panic!("tried to despawn an entity that was not finished."),
    }
  }

  /// Get the data associated with the given entity.
  pub fn get(&self, entity: Entity) -> &EntityAssoc {
    match self.assocs.get(&entity) {
      Some(it) => it,
      None => panic!("tried to get an unfinished entity"),
    }
  }

  pub fn len(&self) -> usize {
    self.assocs.len()
  }

  pub fn liveness(&self, entity: Entity) -> EntityLiveness {
    let allocator = self.allocator.read().unwrap();
    match (
      self.assocs.contains_key(&entity),
      allocator.contains(entity.0),
    ) {
      (true, true) => EntityLiveness::Alive,
      (false, true) => EntityLiveness::PartiallySpawned,
      (false, false) => EntityLiveness::Dead,
      (true, false) => {
        panic!(
          "{:?} was in the assocs but not the allocator somehow",
          entity
        )
      }
    }
  }

  pub fn len_of(&self, entity: Entity) -> usize {
    let assoc = self
      .assocs
      .get(&entity)
      .expect("tried to get the len of a dead entity");
    assoc.len()
  }

  pub fn iter(&self) -> EntityIter<'_> {
    EntityIter {
      iter: self.assocs.keys().copied(),
    }
  }
}

/// Data stored under each entity.
///
/// The internals of this are private and you really shouldn't be using it;
/// I need to make it public for `Query` though.
#[doc(hidden)]
pub struct EntityAssoc {
  components: IndexMap<TypeIdWrapper, ComponentEntry, ahash::RandomState>,
}

impl EntityAssoc {
  pub(crate) fn new(
    components: impl IntoIterator<Item = Box<dyn Component>>,
  ) -> Self {
    let components = components
      .into_iter()
      .map(|comp| ((*comp).type_id_wrapper(), RwLock::new(comp)))
      .collect();
    Self { components }
  }

  pub(crate) fn empty() -> Self {
    Self {
      components: IndexMap::default(),
    }
  }

  /// Iterate in increasing order of priority
  pub(crate) fn iter(
    &self,
  ) -> impl Iterator<Item = (TypeIdWrapper, &ComponentEntry)> + '_ {
    self.components.iter().map(|(tid, comp)| (*tid, comp))
  }

  pub(crate) fn into_iter(
    self,
  ) -> impl Iterator<Item = (TypeIdWrapper, ComponentEntry)> {
    self.components.into_iter()
  }

  pub(crate) fn len(&self) -> usize {
    self.components.len()
  }

  pub(crate) fn components(
    &self,
  ) -> &IndexMap<TypeIdWrapper, ComponentEntry, ahash::RandomState> {
    &self.components
  }
}

/// How each component is stored. Right now this uses naive locking; in the future we might
/// do something fancier.
pub(crate) type ComponentEntry = RwLock<Box<dyn Component>>;