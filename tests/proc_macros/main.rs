#![cfg(feature = "derive")]

use palkia::prelude::*;

#[derive(Message)]
struct MsgTesting {
  flag: bool,
}

struct MyComponent;

impl Component for MyComponent {
  fn register_handlers(builder: HandlerBuilder<Self>) -> HandlerBuilder<Self>
  where
    Self: Sized,
  {
    builder.handle_read(|_, mut msg: MsgTesting, _, _| {
      msg.flag = true;
      msg
    })
  }
}

#[test]
fn derive_message() {
  let mut world = World::new();
  world.register_component::<MyComponent>();

  let testee = world.spawn_1(MyComponent);
  let msg = world.dispatch(testee, MsgTesting { flag: false });
  assert!(msg.flag);
}
