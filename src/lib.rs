#![doc = include_str!("../README.md")]

mod allocator;
pub mod builder;
mod callback;
pub mod component;
pub mod entities;
pub mod messages;
pub mod query;
pub mod resource;
pub mod world;

use std::any::{self, TypeId};
use std::fmt::Debug;
use std::hash::{Hash, Hasher};

use downcast::Any;
use prelude::Entity;

#[derive(Clone, Copy)]
struct TypeIdWrapper {
    tid: TypeId,
    type_name: &'static str,
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

fn loop_panic(perpetrator: Entity) -> ! {
    panic!("{:?} sent an event to one of its own components when it was mutably borrowed, probably via a loop of events. check the stacktrace.", perpetrator)
}

pub mod prelude {
    pub use crate::builder::{EntityBuilder, ImmediateEntityBuilder, LazyEntityBuilder};
    pub use crate::component::{Component, HandlerBuilder};
    pub use crate::entities::Entity;
    pub use crate::messages::{Message, MsgHandlerRead, MsgHandlerWrite};
    pub use crate::query::Query;
    pub use crate::resource::{ReadResource, Resource, ResourceLookupError, WriteResource};
    pub use crate::world::{World, WorldAccess};
}
