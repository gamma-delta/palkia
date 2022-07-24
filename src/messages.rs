use downcast::{downcast, AnySync};

use crate::prelude::{Component, Entity, WorldAccess};

/// Data that is threaded through components.
///
/// When a message is dispatched to an entity, it goes through its components. A component with a handler for this type
/// registered with [`World::register_component`] gets its listener called, and then the updated event gets passed to the next
/// component ... and so on. Then, it's returned to the dispatcher.
pub trait Message: AnySync {}
downcast!(dyn Message);

/// A message handler that only needs immutable access to the component.
pub type MsgHandlerRead<C, E> = fn(this: &C, event: E, owner: Entity, access: &WorldAccess) -> E;
/// A message handler that needs mutable access to the component.
pub type MsgHandlerWrite<C, E> =
    fn(this: &mut C, event: E, owner: Entity, access: &WorldAccess) -> E;

pub(crate) enum MsgHandlerInner {
    Read(
        Box<
            dyn Sync
                + Fn(&dyn Component, Box<dyn Message>, Entity, &WorldAccess) -> Box<dyn Message>,
        >,
    ),
    Write(
        Box<
            dyn Sync
                + Fn(&mut dyn Component, Box<dyn Message>, Entity, &WorldAccess) -> Box<dyn Message>,
        >,
    ),
}
