use std::any;
use std::collections::BTreeMap;
use std::marker::PhantomData;

use downcast::{downcast_sync, AnySync};

use crate::callback::{CallbackWorldAccess, OnCreateCallback, OnRemoveCallback};
use crate::messages::{Message, MsgHandlerInner, MsgHandlerRead, MsgHandlerWrite};
use crate::prelude::{Entity, ListenerWorldAccess};
use crate::TypeIdWrapper;

/// Something attached to an [`Entity`] that gives it its behavior.
pub trait Component: AnySync {
    /// Register what message types this listens to and what it does with them.
    ///
    /// See [`HandlerBuilder`] for more information.
    fn register_handlers(builder: HandlerBuilder<Self>) -> HandlerBuilder<Self>
    where
        Self: Sized;
}
downcast_sync!(dyn Component);

/// Builder that registers listeners and callbacks to components.
#[must_use = "does nothing until .build() is called"]
pub struct HandlerBuilder<C: Component + ?Sized> {
    /// Maps event types to their handlers.
    pub(crate) handlers: BTreeMap<TypeIdWrapper, MsgHandlerInner>,
    pub(crate) create_cb: Option<OnCreateCallback>,
    pub(crate) remove_cb: Option<OnRemoveCallback>,

    phantom: PhantomData<C>,
}

impl<C: Component> HandlerBuilder<C> {
    pub(crate) fn new() -> Self {
        Self {
            handlers: BTreeMap::new(),
            create_cb: None,
            remove_cb: None,
            phantom: PhantomData,
        }
    }

    /// Tell the world to send the given type of message to this component to be handled with read access.
    pub fn handle_read<M: Message>(mut self, handler: MsgHandlerRead<C, M>) -> Self {
        let tid = TypeIdWrapper::of::<M>();
        if self.handlers.contains_key(&tid) {
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
            let component: &C = unsafe { component.downcast_ref().unwrap_unchecked() };
            // SAFETY: this will only be called with the right concrete type, checked by the type ID
            let event: Box<M> = unsafe { event.downcast().unwrap_unchecked() };
            let res = handler(component, *event, entity, access);
            Box::new(res) as _
        };
        self.handlers
            .insert(tid, MsgHandlerInner::Read(Box::new(clo)));
        self
    }

    /// Tell the world to send the given type of message to this component to be handled with write access.
    pub fn handle_write<M: Message>(mut self, handler: MsgHandlerWrite<C, M>) -> Self {
        let tid = TypeIdWrapper::of::<M>();
        if self.handlers.contains_key(&tid) {
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
            let component: &mut C = unsafe { component.downcast_mut().unwrap_unchecked() };
            // SAFETY: this will only be called with the right concrete type, checked by the type ID
            let event: Box<M> = unsafe { event.downcast().unwrap_unchecked() };
            let res = handler(component, *event, entity, access);
            Box::new(res) as _
        };
        self.handlers
            .insert(tid, MsgHandlerInner::Write(Box::new(clo)));
        self
    }

    /// Register a callback function to be called when an entity with components of the given type is inserted into the world.
    ///
    /// These are called immediately after spawning an entity with a world, and during [`World::finalize`],
    /// for each new instance of that component type.
    ///
    /// Panics if another insert callback has already been registered to this component type or if the component
    /// type has not been registered.
    pub fn register_create_callback(mut self, cb: fn(&C, Entity, &CallbackWorldAccess)) -> Self {
        if self.create_cb.is_some() {
            panic!(
                "a create callback for {:?} already exists",
                any::type_name::<C>()
            );
        }
        let clo = move |comp: &dyn Component, e: Entity, access: &CallbackWorldAccess| {
            // SAFETY: this will only ever be called with a component of the right concrete type
            let concrete_comp: &C = unsafe { comp.downcast_ref().unwrap_unchecked() };
            cb(concrete_comp, e, access);
        };
        let clo = Box::new(clo);

        self.create_cb = Some(clo);
        self
    }

    /// Register a callback function to be called when an entity with components of the given type
    /// is removed from the world.
    ///
    /// These are called immediately after deleting an entity from a world, and during [`World::finalize`],
    /// for each new instance of that component type.
    ///
    /// Panics if another removal callback has already been registered to this component type or if the component
    /// type has not been registered.
    ///
    /// **NOTE THAT** the entity given in the callback will ALWAYS be dead.
    pub fn register_remove_callback(mut self, cb: fn(C, Entity, &CallbackWorldAccess)) -> Self {
        if self.remove_cb.is_some() {
            panic!(
                "a remove callback for {:?} already exists",
                any::type_name::<C>()
            );
        }
        let clo = move |comp: Box<dyn Component>, e: Entity, access: &CallbackWorldAccess| {
            // SAFETY: this will only ever be called with a component of the right concrete type
            let concrete_comp: C = unsafe { *comp.downcast().unwrap_unchecked() };
            cb(concrete_comp, e, access);
        };
        let clo = Box::new(clo);

        self.remove_cb = Some(clo);
        self
    }
}
