use std::{
  fmt::Display,
  marker::PhantomData,
  ops::{Deref, DerefMut},
  sync::{RwLockReadGuard, RwLockWriteGuard},
};

use crate::TypeIdWrapper;

use super::Resource;

/// Opaque wrapper for an immutable reference to a resource.
pub struct ReadResource<'a, T: ?Sized>(
  RwLockReadGuard<'a, Box<dyn Resource>>,
  PhantomData<T>,
);

impl<'a, T: ?Sized> ReadResource<'a, T> {
  pub(crate) fn new(guard: RwLockReadGuard<'a, Box<dyn Resource>>) -> Self {
    Self(guard, PhantomData)
  }
}

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

impl<'a, T: ?Sized> WriteResource<'a, T> {
  pub(crate) fn new(lock: RwLockWriteGuard<'a, Box<dyn Resource>>) -> Self {
    Self(lock, PhantomData)
  }
}

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
