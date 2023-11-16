use std::{
  collections::BTreeMap,
  sync::{RwLock, TryLockError},
};

use ahash::AHashMap;
use generational_arena::Arena;
use indexmap::IndexMap;

use crate::{
  entities::EntityIter,
  prelude::{Component, Entity, EntityLiveness},
  resource::{
    ReadResource, Resource, ResourceLookupError, ResourceLookupErrorKind,
    WriteResource,
  },
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

/// World storage for the resources
pub(crate) struct ResourceMap {
  map: BTreeMap<TypeIdWrapper, RwLock<Box<dyn Resource>>>,
}

impl ResourceMap {
  pub fn new() -> Self {
    Self {
      map: BTreeMap::new(),
    }
  }

  /// With a mutable reference, get a value from the map directly.
  ///
  /// If the value is poisoned, silently return `None`.
  pub fn get<T: Resource>(&mut self) -> Option<&mut T> {
    let resource = self.map.get_mut(&TypeIdWrapper::of::<T>())?;
    resource
      .get_mut()
      .ok()
      .map(|res| res.downcast_mut().unwrap())
  }

  /// With a mutable reference, get a value from the map directly.
  ///
  /// If the value is poisoned, silently return `None`.
  pub fn remove<T: Resource>(&mut self) -> Option<T> {
    let resource = self.map.remove(&TypeIdWrapper::of::<T>())?;
    match resource.into_inner() {
      Ok(it) => Some(*it.downcast().unwrap()),
      Err(_) => None,
    }
  }

  pub fn read<T: Resource>(
    &self,
  ) -> Result<ReadResource<'_, T>, ResourceLookupError> {
    let tid = TypeIdWrapper::of::<T>();

    let result = 'try_at_home: {
      let Some(resource) = self.map.get(&tid) else {
        break 'try_at_home Err(ResourceLookupErrorKind::NotFound);
      };

      let lock = match resource.try_read() {
        Ok(it) => it,
        Err(TryLockError::WouldBlock) => {
          break 'try_at_home Err(ResourceLookupErrorKind::Locked)
        }
        Err(TryLockError::Poisoned(_)) => {
          break 'try_at_home Err(ResourceLookupErrorKind::Poisoned)
        }
      };

      Ok(ReadResource::new(lock))
    };
    result.map_err(|kind| ResourceLookupError { tid, kind })
  }

  pub fn write<T: Resource>(
    &self,
  ) -> Result<WriteResource<'_, T>, ResourceLookupError> {
    let tid = TypeIdWrapper::of::<T>();

    let result = 'try_at_home: {
      let Some(resource) = self.map.get(&tid) else {
        break 'try_at_home Err(ResourceLookupErrorKind::NotFound);
      };
      let lock = match resource.try_write() {
        Ok(it) => it,
        Err(TryLockError::WouldBlock) => {
          break 'try_at_home Err(ResourceLookupErrorKind::Locked)
        }
        Err(TryLockError::Poisoned(_)) => {
          break 'try_at_home Err(ResourceLookupErrorKind::Poisoned)
        }
      };

      Ok(WriteResource::new(lock))
    };
    result.map_err(|kind| ResourceLookupError { tid, kind })
  }

  pub fn insert<T: Resource>(&mut self, resource: T) -> Option<T> {
    self
      .map
      .insert(
        TypeIdWrapper::of::<T>(),
        RwLock::new(Box::new(resource) as _),
      )
      .map(|old| *old.into_inner().unwrap().downcast().unwrap())
  }

  pub fn insert_raw(
    &mut self,
    resource: Box<dyn Resource>,
  ) -> Option<Box<dyn Resource>> {
    let tid = (*resource).type_id_wrapper();
    self
      .map
      .insert(tid, RwLock::new(resource))
      .map(|old| old.into_inner().unwrap())
  }

  pub fn contains<T: Resource>(&self) -> bool {
    self.map.contains_key(&TypeIdWrapper::of::<T>())
  }

  pub fn iter(
    &self,
  ) -> impl Iterator<Item = (TypeIdWrapper, &RwLock<Box<dyn Resource>>)> + '_
  {
    self.map.iter().map(|(tid, res)| (*tid, res))
  }

  pub fn len(&self) -> usize {
    self.map.len()
  }
}
