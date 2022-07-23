use std::collections::BTreeMap;
use std::marker::PhantomData;

use downcast::{downcast_sync, AnySync};

use crate::events::{Event, EventListener, EventListenerRead, EventListenerWrite};
use crate::prelude::{Entity, WorldAccess};
use crate::TypeIdWrapper;

/// Something attached to an [`Entity`] that gives it its behavior.
pub trait Component: AnySync {
    fn register_listeners(builder: ListenerBuilder<Self>) -> ListenerBuilder<Self>
    where
        Self: Sized;
}
downcast_sync!(dyn Component);

#[must_use = "if you don't need to register any listeners to this component type, consider not using it at all"]
pub struct ListenerBuilder<C: Component + ?Sized> {
    /// Maps event types to their handlers.
    pub(crate) listeners: BTreeMap<TypeIdWrapper, EventListener>,
    phantom: PhantomData<C>,
}

impl<C: Component> ListenerBuilder<C> {
    pub(crate) fn new() -> Self {
        Self {
            listeners: BTreeMap::new(),
            phantom: PhantomData,
        }
    }

    /// Tell the world to send the given type of event to this component to be handled with read access.
    pub fn listen_read<E: Event>(mut self, listener: EventListenerRead<C, E>) -> Self {
        let clo = move |component: &dyn Component,
                        event: Box<dyn Event>,
                        entity: Entity,
                        access: &WorldAccess| {
            let component: &C = component.downcast_ref().unwrap();
            let event: Box<E> = event.downcast().unwrap();
            let res = listener(component, *event, entity, access);
            Box::new(res) as _
        };
        self.listeners
            .insert(TypeIdWrapper::of::<E>(), EventListener::Read(Box::new(clo)));
        self
    }

    /// Tell the world to send the given type of event to this component to be handled with write access.
    pub fn listen_write<E: Event>(mut self, listener: EventListenerWrite<C, E>) -> Self {
        let clo = move |component: &mut dyn Component,
                        event: Box<dyn Event>,
                        entity: Entity,
                        access: &WorldAccess| {
            let component: &mut C = component.downcast_mut().unwrap();
            let event: Box<E> = event.downcast().unwrap();
            let res = listener(component, *event, entity, access);
            Box::new(res) as _
        };
        self.listeners.insert(
            TypeIdWrapper::of::<E>(),
            EventListener::Write(Box::new(clo)),
        );
        self
    }
}
