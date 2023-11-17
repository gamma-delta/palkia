//! Data attachable to [entities](crate::entities::Entity) that control its behavior by listening to [messages](crate::messages::Message).

use std::{any, marker::PhantomData};

use downcast::{downcast, Any};
use serde::{de::DeserializeOwned, Serialize};

use crate::{
  callback::{CallbackWorldAccess, Callbacks},
  messages::{Message, MsgHandlerInner, MsgHandlerRead, MsgHandlerWrite},
  prelude::{Entity, ListenerWorldAccess},
  vtablesathome::{self, ComponentVtable, DeserializeFn},
  TypeIdWrapper,
};

/// Something attached to an [`Entity`] that gives it its behavior.
///
/// Components all have a "friendly name". This is the name used to read it from
/// a blueprint, and used in ser/de as well. By default it is the last field in
/// [`std::any::type_name`] (split on `::`).
pub trait Component: Any + erased_serde::Serialize {
  /// Register what message types this listens to and what it does with them.
  fn register(builder: ComponentRegisterer<Self>) -> ComponentRegisterer<Self>
  where
    Self: Sized;
}
downcast!(dyn Component);

/// Builder that registers listeners and callbacks and stuff to components.
#[must_use]
pub struct ComponentRegisterer<C: Component> {
  inner: ComponentRegistererErased,
  phantom: PhantomData<C>,
}

impl<C> ComponentRegisterer<C>
where
  C: Component + Serialize + DeserializeOwned,
{
  /// Tell the world to send the given type of message to this component to be handled with read access.
  pub fn handle_read<M: Message>(
    mut self,
    handler: MsgHandlerRead<C, M>,
  ) -> Self {
    let tid = TypeIdWrapper::of::<M>();
    if self.inner.handlers.contains_key(&tid) {
      panic!(
        "already registered message type {:?} to component type {:?}",
        tid.type_name,
        TypeIdWrapper::of::<C>().type_name
      );
    }

    let clo = move |component: &dyn Component,
                    event: Box<dyn Message>,
                    entity: Entity,
                    access: &ListenerWorldAccess| {
      // SAFETY: this will only be called with the right concrete type, checked by the type ID
      let component: &C =
        unsafe { component.downcast_ref().unwrap_unchecked() };
      // SAFETY: this will only be called with the right concrete type, checked by the type ID
      let event: Box<M> = unsafe { event.downcast().unwrap_unchecked() };
      let res = handler(component, *event, entity, access);
      Box::new(res) as _
    };
    self
      .inner
      .handlers
      .insert(tid, MsgHandlerInner::Read(Box::new(clo)));
    self
  }

  /// Tell the world to send the given type of message to this component to be handled with write access.
  pub fn handle_write<M: Message>(
    mut self,
    handler: MsgHandlerWrite<C, M>,
  ) -> Self {
    let tid = TypeIdWrapper::of::<M>();
    if self.inner.handlers.contains_key(&tid) {
      panic!(
        "already registered message type {:?} to component type {:?}",
        tid.type_name,
        TypeIdWrapper::of::<C>().type_name
      );
    }
    let clo = move |component: &mut dyn Component,
                    event: Box<dyn Message>,
                    entity: Entity,
                    access: &ListenerWorldAccess| {
      // SAFETY: this will only be called with the right concrete type, checked by the type ID
      let component: &mut C =
        unsafe { component.downcast_mut().unwrap_unchecked() };
      // SAFETY: this will only be called with the right concrete type, checked by the type ID
      let event: Box<M> = unsafe { event.downcast().unwrap_unchecked() };
      let res = handler(component, *event, entity, access);
      Box::new(res) as _
    };
    self
      .inner
      .handlers
      .insert(tid, MsgHandlerInner::Write(Box::new(clo)));
    self
  }

  /// Register a callback function to be called when an entity with components of the given type is inserted into the world.
  ///
  /// These are called immediately after spawning an entity with a world, and during [`World::finalize`][crate::world::World::finalize],
  /// for each new instance of that component type.
  ///
  /// Panics if another insert callback has already been registered to this component type or if the component
  /// type has not been registered.
  pub fn register_create_callback(
    mut self,
    cb: fn(&C, Entity, &CallbackWorldAccess),
  ) -> Self {
    if self.inner.create_cb.is_some() {
      panic!(
        "a create callback for {:?} already exists",
        any::type_name::<C>()
      );
    }
    let clo = move |comp: &dyn Component,
                    e: Entity,
                    access: &CallbackWorldAccess| {
      // SAFETY: this will only ever be called with a component of the right concrete type
      let concrete_comp: &C = unsafe { comp.downcast_ref().unwrap_unchecked() };
      cb(concrete_comp, e, access);
    };
    let clo = Box::new(clo);

    self.inner.create_cb = Some(clo);
    self
  }

  /// Register a callback function to be called when an entity with components of the given type
  /// is removed from the world.
  ///
  /// These are called immediately after deleting an entity from a world, and during [`World::finalize`][crate::world::World::finalize],
  /// for each new instance of that component type.
  ///
  /// Panics if another removal callback has already been registered to this component type or if the component
  /// type has not been registered.
  ///
  /// **NOTE THAT** the entity given in the callback will ALWAYS be dead.
  pub fn register_remove_callback(
    mut self,
    cb: fn(C, Entity, &CallbackWorldAccess),
  ) -> Self {
    if self.inner.remove_cb.is_some() {
      panic!(
        "a remove callback for {:?} already exists",
        any::type_name::<C>()
      );
    }
    let clo = move |comp: Box<dyn Component>,
                    e: Entity,
                    access: &CallbackWorldAccess| {
      // SAFETY: this will only ever be called with a component of the right concrete type
      let concrete_comp: C = unsafe { *comp.downcast().unwrap_unchecked() };
      cb(concrete_comp, e, access);
    };
    let clo = Box::new(clo);

    self.inner.remove_cb = Some(clo);
    self
  }

  /// Manually set the friendly name of this component to something other
  /// than the default (a best-effort guess at the type name based on
  /// `std::any::type_name`).
  pub fn set_friendly_name(mut self, name: &'static str) -> Self {
    if let Some(ono) = self.inner.friendly_name.replace(name) {
      panic!(
        "tried to set the friendly name of component {} to {:?} but was already set to {:?}",
        std::any::type_name::<C>(),
        name,
        ono,
      );
    }
    self
  }

  #[doc(hidden)]
  pub fn into_vtable(self) -> ComponentVtable {
    let friendly_name = self
      .inner
      .friendly_name
      .unwrap_or_else(vtablesathome::default_friendly_type_name::<C>);

    let callbacks = match (self.inner.create_cb, self.inner.remove_cb) {
      (None, None) => None,
      (None, Some(remove)) => Some(Callbacks::Remove(remove)),
      (Some(create), None) => Some(Callbacks::Create(create)),
      (Some(create), Some(remove)) => Some(Callbacks::Both(create, remove)),
    };

    let deser = |deser: &mut dyn erased_serde::Deserializer| -> erased_serde::Result<Box<dyn Component>> {
      let this = C::deserialize(deser)?;
      Ok(Box::new(this) as _)
    }
      as DeserializeFn<dyn Component>;

    ComponentVtable {
      tid: TypeIdWrapper::of::<C>(),

      friendly_name,
      msg_table: self.inner.handlers,
      callbacks,
      deser,
    }
  }
}

#[doc(hidden)]
pub mod __private {
  use std::{collections::BTreeMap, marker::PhantomData};

  use super::{Component, ComponentRegisterer};
  use crate::{
    callback::{OnCreateCallback, OnRemoveCallback},
    messages::MsgHandlerInner,
    TypeIdWrapper,
  };

  pub struct ComponentRegistererErased {
    pub(crate) friendly_name: Option<&'static str>,
    /// Maps event types to their handlers.
    pub(crate) handlers: BTreeMap<TypeIdWrapper, MsgHandlerInner>,
    pub(crate) create_cb: Option<OnCreateCallback>,
    pub(crate) remove_cb: Option<OnRemoveCallback>,
  }

  impl ComponentRegistererErased {
    pub(crate) fn new() -> Self {
      Self {
        handlers: BTreeMap::new(),
        create_cb: None,
        remove_cb: None,
        friendly_name: None,
      }
    }

    pub fn wrap<C: Component>(self) -> ComponentRegisterer<C> {
      ComponentRegisterer {
        inner: self,
        phantom: PhantomData,
      }
    }
  }
}
pub(crate) use __private::*;

/// Longhand component register macro. You can call this as
/// `manually_register_component(MyComponent)` if you're allergic to
/// attribute macros for some reason.
#[macro_export]
macro_rules! manually_register_component {
  ($component_ty:ty) => {
    $crate::__private::paste! {
      #[doc(hidden)]
      #[allow(non_snake_case)]
      #[palkia::__private::linkme::distributed_slice(
          palkia::__private::COMPONENT_REGISTRATORS
      )]
      #[linkme(crate = palkia::__private::linkme)]
      fn [< secret_register_ $component_ty>]
        (regi: $crate::__private::ComponentRegistererErased)
        -> $crate::__private::ComponentVtable {
        let wrapped = regi.wrap();
        <$component_ty as $crate::component::Component>::register(wrapped).into_vtable()
      }
    }
  };
}
