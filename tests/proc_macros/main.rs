#![cfg(feature = "derive")]

use palkia::prelude::*;

#[derive(Message)]
struct MsgTesting {
  flag: bool,
}

#[derive(Resource)]
struct MyResource {
  flag: bool,
}

struct MyComponent;

impl Component for MyComponent {
  fn register_handlers(builder: HandlerBuilder<Self>) -> HandlerBuilder<Self>
  where
    Self: Sized,
  {
    builder
      .handle_read(|_, mut msg: MsgTesting, _, _| {
        msg.flag = true;
        msg
      })
      .register_create_callback(|_, _, access| {
        let mut res = access.write_resource::<MyResource>().unwrap();
        res.flag = true;
      })
  }
}

#[test]
fn derive_message() {
  let mut world = World::new();
  world.register_component::<MyComponent>();

  world.insert_resource(MyResource { flag: false });

  let testee = world.spawn_1(MyComponent);
  let msg = world.dispatch(testee, MsgTesting { flag: false });
  assert!(msg.flag);

  let res = world.get_resource::<MyResource>().unwrap();
  assert!(res.flag);
}
