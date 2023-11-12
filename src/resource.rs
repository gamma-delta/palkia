//! Singleton data stored on the world.

use std::{
  collections::BTreeMap,
  fmt::Display,
  marker::PhantomData,
  ops::{Deref, DerefMut},
  sync::{RwLock, RwLockReadGuard, RwLockWriteGuard, TryLockError},
};

use downcast::{downcast, Any};

use crate::{ToTypeIdWrapper, TypeIdWrapper};

/// A resource is data attached to a [World](crate::world::World). There is up to one instance of a given Resource type
/// per world.
///
/// This is handy for things you need across many entities, like position caches, assets, settings, save data ...
/// anything that wouldn't make sense to have more than one of.
///
/// The trait is more or less a marker trait, so you don't accidentally put the wrong thing in worlds.
pub trait Resource: Any {}
downcast!(dyn Resource);

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

      Ok(ReadResource(lock, PhantomData))
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

      Ok(WriteResource(lock, PhantomData))
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

/// Opaque wrapper for an immutable reference to a resource.
pub struct ReadResource<'a, T: ?Sized>(
  RwLockReadGuard<'a, Box<dyn Resource>>,
  PhantomData<T>,
);
impl<'a, T: Resource> Deref for ReadResource<'a, T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    let the_box = &*self.0;
    the_box.downcast_ref().unwrap()
  }
}

/// Opaque wrapper for a mutable reference to a resource.
pub struct WriteResource<'a, T: ?Sized>(
  RwLockWriteGuard<'a, Box<dyn Resource>>,
  PhantomData<T>,
);
impl<'a, T: Resource> Deref for WriteResource<'a, T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    let the_box = &*self.0;
    the_box.downcast_ref().unwrap()
  }
}
impl<'a, T: Resource> DerefMut for WriteResource<'a, T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    let the_box = &mut *self.0;
    the_box.downcast_mut().unwrap()
  }
}

#[derive(Debug, Clone, Copy)]
pub struct ResourceLookupError {
  pub tid: TypeIdWrapper,
  pub kind: ResourceLookupErrorKind,
}

/// Problems when trying to get a resource from a world.
#[derive(Debug, Clone, Copy)]
pub enum ResourceLookupErrorKind {
  NotFound,
  /// Either there's already an immutable reference to that resource and you tried to get a mutable one,
  /// or there was already a mutable reference to that resource and you tried to get an immutable one.
  Locked,
  /// The lock was poisoned; something panicked while the lock was held.
  ///
  /// This should *probably* never happen.
  Poisoned,
}

impl Display for ResourceLookupError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self.kind {
            ResourceLookupErrorKind::NotFound => write!(f, "a resource of type {} was not found", &self.tid.type_name),
            ResourceLookupErrorKind::Locked => write!(f, "the resource of type {} was found, but it was borrowed in such a way it could not be reborrowed", &self.tid.type_name),
            ResourceLookupErrorKind::Poisoned => write!(f, "the resource of type {} was found, but its lock was poisoned -- what on earth are you doing‽", &self.tid.type_name)
        }
  }
}
