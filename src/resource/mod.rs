//! Singleton data stored on the world.

mod storage;
use std::marker::PhantomData;

pub use storage::*;

use downcast::{downcast, Any};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::{
  vtablesathome::{self, DeserializeFn, ResourceVtable},
  TypeIdWrapper,
};

/// A resource is data attached to a [World](crate::world::World). There is up to one instance of a given Resource type
/// per world.
///
/// This is handy for things you need across many entities, like position caches, assets, settings, save data ...
/// anything that wouldn't make sense to have more than one of.
///
pub trait Resource: Any + erased_serde::Serialize {
  fn register(builder: ResourceRegisterer<Self>) -> ResourceRegisterer<Self>
  where
    Self: Sized,
  {
    builder
  }
}
downcast!(dyn Resource);

pub struct ResourceRegisterer<R> {
  inner: ResourceRegistererErased,
  phantom: PhantomData<R>,
}

impl<R> ResourceRegisterer<R>
where
  R: Resource + Serialize + DeserializeOwned,
{
  pub fn set_friendly_name(mut self, name: &'static str) -> Self {
    if let Some(ono) = self.inner.friendly_name.replace(name) {
      panic!(
        "tried to set the friendly name of resource {} to {:?} but was already set to {:?}",
        std::any::type_name::<R>(),
        name,
        ono,
      );
    }
    self
  }

  #[doc(hidden)]
  pub fn into_vtable(self) -> ResourceVtable {
    let friendly_name = self
      .inner
      .friendly_name
      .unwrap_or_else(vtablesathome::default_friendly_type_name::<R>);

    let deser = |deser: &mut dyn erased_serde::Deserializer| -> erased_serde::Result<Box<dyn Resource>> {
      let this = R::deserialize(deser)?;
      Ok(Box::new(this) as _)
    }
      as DeserializeFn<dyn Resource>;

    ResourceVtable {
      tid: TypeIdWrapper::of::<R>(),
      friendly_name,
      deser,
    }
  }
}

#[doc(hidden)]
pub mod __private {
  use std::marker::PhantomData;

  use super::{Resource, ResourceRegisterer};

  pub struct ResourceRegistererErased {
    pub(crate) friendly_name: Option<&'static str>,
  }

  impl ResourceRegistererErased {
    pub(crate) fn new() -> Self {
      Self {
        friendly_name: None,
      }
    }

    pub fn wrap<R: Resource>(self) -> ResourceRegisterer<R> {
      ResourceRegisterer {
        inner: self,
        phantom: PhantomData,
      }
    }
  }
}
pub use __private::*;

/// Longhand resource register macro. You can call this as
/// `manually_register_resource(MyResource)` if you're allergic to
/// attribute macros for some reason.
#[macro_export]
macro_rules! manually_register_resource {
  ($res_ty:ty) => {
    $crate::__private::paste! {
      #[doc(hidden)]
      #[allow(non_snake_case)]
      #[$crate::__private::distributed_slice(
          $crate::__private::RESOURCE_REGISTRATORS
      )]
      fn [< secret_register_ $res_ty>]
        (regi: $crate::__private::ResourceRegistererErased)
        -> $crate::__private::ResourceVtable {
        let wrapped = regi.wrap();
        <$res_ty as $crate::resource::Resource>::register(wrapped).into_vtable()
      }
    }
  };
}
