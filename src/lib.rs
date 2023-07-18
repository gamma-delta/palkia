#![doc = include_str!("../README.md")]
#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod access;
pub mod builder;
pub mod callback;
pub mod component;
pub mod entities;
pub mod fabricator;
pub mod messages;
pub mod query;
pub mod resource;
pub mod world;

#[cfg(feature = "serde")]
#[cfg_attr(docsrs, doc(cfg(feature = "serde")))]
pub mod serde;

use std::{
  any::{self, TypeId},
  fmt::Debug,
  hash::{Hash, Hasher},
};

use downcast::Any;
use prelude::Entity;

#[derive(Clone, Copy)]
/// Wrapper for a [`TypeId`] that also stores the name of the type, to aid in debugging
/// and for nicer error messages.
///
/// You should probably not be using this...
pub struct TypeIdWrapper {
  pub tid: TypeId,
  pub type_name: &'static str,
}

impl std::ops::Deref for TypeIdWrapper {
  type Target = TypeId;

  fn deref(&self) -> &Self::Target {
    &self.tid
  }
}

impl TypeIdWrapper {
  pub fn of<T: 'static>() -> Self {
    Self {
      tid: TypeId::of::<T>(),
      type_name: any::type_name::<T>(),
    }
  }
}

impl PartialEq for TypeIdWrapper {
  fn eq(&self, other: &Self) -> bool {
    self.tid == other.tid
  }
}

impl Eq for TypeIdWrapper {}

impl PartialOrd for TypeIdWrapper {
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for TypeIdWrapper {
  fn cmp(&self, other: &Self) -> std::cmp::Ordering {
    self.tid.cmp(&other.tid)
  }
}

impl Hash for TypeIdWrapper {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.tid.hash(state);
  }
}

impl Debug for TypeIdWrapper {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let mut dbs = f.debug_tuple("TypeIdWrapper");

    #[cfg(debug_assertions)]
    dbs.field(&self.type_name);

    dbs.finish()
  }
}

trait ToTypeIdWrapper {
  fn type_id_wrapper(&self) -> TypeIdWrapper;
}

impl<T: Any> ToTypeIdWrapper for T
where
  T: ?Sized,
{
  fn type_id_wrapper(&self) -> TypeIdWrapper {
    TypeIdWrapper {
      tid: self.type_id(),
      type_name: self.type_name(),
    }
  }
}

fn loop_panic(perpetrator: Entity, comp_tid: TypeIdWrapper) -> ! {
  panic!("{:?} sent an event to one of its own components of type {} when it was mutably borrowed, probably via a loop of events. check the stacktrace.", perpetrator, comp_tid.type_name)
}

pub mod prelude {
  //! Handy module to glob-import and get everything in the crate.
  #[cfg(feature = "serde")]
  pub use crate::serde::{
    EntityDeContext, EntitySerContext, ResourceDeContext, ResourceSerContext,
    SerKey, WorldSerdeInstructions,
  };
  pub use crate::{
    access::{
      AccessDispatcher, AccessEntityStats, AccessQuery, AccessResources,
    },
    builder::EntityBuilder,
    callback::CallbackWorldAccess,
    component::{Component, HandlerBuilder},
    entities::{Entity, EntityLiveness},
    messages::{ListenerWorldAccess, Message, MsgHandlerRead, MsgHandlerWrite},
    query::Query,
    resource::{ReadResource, Resource, ResourceLookupError, WriteResource},
    world::World,
  };
}
