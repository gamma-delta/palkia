use std::collections::BTreeMap;
use std::marker::PhantomData;

use downcast::{downcast_sync, AnySync};

use crate::messages::{Message, MsgHandlerInner, MsgHandlerRead, MsgHandlerWrite};
use crate::prelude::{Entity, WorldAccess};
use crate::TypeIdWrapper;

/// Something attached to an [`Entity`] that gives it its behavior.
pub trait Component: AnySync {
    /// Register what message types this listens to and what it does with them.
    ///
    /// See [`HandlerBuilder`] for more information.
    fn register_handlers(builder: HandlerBuilder<Self>) -> HandlerBuilder<Self>
    where
        Self: Sized;

    /// Get the priority of this component. Components with a lower priority number will have events sent to them first.
    ///
    /// Two components on the same entity must not have the same priority, or it will panic.
    fn priority() -> u64
    where
        Self: Sized;
}
downcast_sync!(dyn Component);

#[must_use = "does nothing until .build() is called"]
pub struct HandlerBuilder<C: Component + ?Sized> {
    /// Maps event types to their handlers.
    pub(crate) handlers: BTreeMap<TypeIdWrapper, MsgHandlerInner>,
    phantom: PhantomData<C>,
}

impl<C: Component> HandlerBuilder<C> {
    pub(crate) fn new() -> Self {
        Self {
            handlers: BTreeMap::new(),
            phantom: PhantomData,
        }
    }

    /// Tell the world to send the given type of message to this component to be handled with read access.
    pub fn handle_read<M: Message>(mut self, handler: MsgHandlerRead<C, M>) -> Self {
        let clo = move |component: &dyn Component,
                        event: Box<dyn Message>,
                        entity: Entity,
                        access: &WorldAccess| {
            let component: &C = component.downcast_ref().unwrap();
            let event: Box<M> = event.downcast().unwrap();
            let res = handler(component, *event, entity, access);
            Box::new(res) as _
        };
        self.handlers.insert(
            TypeIdWrapper::of::<M>(),
            MsgHandlerInner::Read(Box::new(clo)),
        );
        self
    }

    /// Tell the world to send the given type of message to this component to be handled with write access.
    pub fn handle_write<M: Message>(mut self, handler: MsgHandlerWrite<C, M>) -> Self {
        let clo = move |component: &mut dyn Component,
                        event: Box<dyn Message>,
                        entity: Entity,
                        access: &WorldAccess| {
            let component: &mut C = component.downcast_mut().unwrap();
            let event: Box<M> = event.downcast().unwrap();
            let res = handler(component, *event, entity, access);
            Box::new(res) as _
        };
        self.handlers.insert(
            TypeIdWrapper::of::<M>(),
            MsgHandlerInner::Write(Box::new(clo)),
        );
        self
    }
}
