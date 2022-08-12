use std::collections::BTreeMap;
use std::fmt::Display;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard, TryLockError};

use downcast::{downcast, AnySync};

use crate::{ToTypeIdWrapper, TypeIdWrapper};

/// Marker trait for resources so you don't accidentally put the wrong thing in worlds.
pub trait Resource: AnySync {}
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

    pub fn read<T: Resource>(&self) -> Result<ReadResource<'_, T>, ResourceLookupError> {
        let resource = match self.map.get(&TypeIdWrapper::of::<T>()) {
            Some(it) => it,
            None => return Err(ResourceLookupError::NotFound),
        };
        let lock = match resource.try_read() {
            Ok(it) => it,
            Err(TryLockError::WouldBlock) => return Err(ResourceLookupError::Locked),
            Err(TryLockError::Poisoned(_)) => return Err(ResourceLookupError::Poisoned),
        };
        Ok(ReadResource(lock, PhantomData))
    }
    pub fn write<T: Resource>(&self) -> Result<WriteResource<'_, T>, ResourceLookupError> {
        let resource = match self.map.get(&TypeIdWrapper::of::<T>()) {
            Some(it) => it,
            None => return Err(ResourceLookupError::NotFound),
        };
        let lock = match resource.try_write() {
            Ok(it) => it,
            Err(TryLockError::WouldBlock) => return Err(ResourceLookupError::Locked),
            Err(TryLockError::Poisoned(_)) => return Err(ResourceLookupError::Poisoned),
        };
        Ok(WriteResource(lock, PhantomData))
    }

    pub fn insert<T: Resource>(&mut self, resource: T) -> Option<T> {
        self.map
            .insert(
                TypeIdWrapper::of::<T>(),
                RwLock::new(Box::new(resource) as _),
            )
            .map(|old| *old.into_inner().unwrap().downcast().unwrap())
    }

    pub fn insert_raw(&mut self, resource: Box<dyn Resource>) -> Option<Box<dyn Resource>> {
        let tid = (*resource).type_id_wrapper();
        self.map
            .insert(tid, RwLock::new(resource))
            .map(|old| old.into_inner().unwrap())
    }

    pub fn contains<T: Resource>(&self) -> bool {
        self.map.contains_key(&TypeIdWrapper::of::<T>())
    }

    pub fn iter(&self) -> impl Iterator<Item = (TypeIdWrapper, &RwLock<Box<dyn Resource>>)> + '_ {
        self.map.iter().map(|(tid, res)| (*tid, res))
    }
}

/// Opaque wrapper for an immutable reference to something in a resource map.
pub struct ReadResource<'a, T: ?Sized>(RwLockReadGuard<'a, Box<dyn Resource>>, PhantomData<T>);
impl<'a, T: Resource> Deref for ReadResource<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        let the_box = &*self.0;
        the_box.downcast_ref().unwrap()
    }
}

/// Opaque wrapper for a mutable reference to something in a resource map.
pub struct WriteResource<'a, T: ?Sized>(RwLockWriteGuard<'a, Box<dyn Resource>>, PhantomData<T>);
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
pub enum ResourceLookupError {
    NotFound,
    Locked,
    Poisoned,
}

impl Display for ResourceLookupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResourceLookupError::NotFound => write!(f, "a resource of that type was not found"),
            ResourceLookupError::Locked => write!(f, "the resource of that type was found, but it was borrowed in such a way it could not be reborrowed"),
            ResourceLookupError::Poisoned => write!(f, "the resource of that type was found, but it was poisoned")
        }
    }
}
