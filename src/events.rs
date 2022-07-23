use downcast::{downcast, AnySync};

use crate::prelude::{Component, Entity, WorldAccess};

/// Data that is threaded through components.
///
/// When an event is dispatched to an entity, it goes through its components. A component with a listener for this type
/// registered with [`World::register_component`] gets its listener called, and then the updated event gets passed to the next
/// component ... and so on. Then, it's returned to the dispatcher.
pub trait Event: AnySync {}
downcast!(dyn Event);

/// An event listener that only needs immutable access to the component.
pub type EventListenerRead<C, E> = fn(this: &C, event: E, owner: Entity, access: &WorldAccess) -> E;
/// An event listener that needs mutable access to the component.
pub type EventListenerWrite<C, E> =
    fn(this: &mut C, event: E, owner: Entity, access: &WorldAccess) -> E;

pub(crate) enum EventListener {
    Read(
        Box<dyn Sync + Fn(&dyn Component, Box<dyn Event>, Entity, &WorldAccess) -> Box<dyn Event>>,
    ),
    Write(
        Box<
            dyn Sync
                + Fn(&mut dyn Component, Box<dyn Event>, Entity, &WorldAccess) -> Box<dyn Event>,
        >,
    ),
}
