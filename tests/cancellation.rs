use palkia::prelude::*;
use serde::{Deserialize, Serialize};

/// sorry, i meant `struct X`
#[derive(Serialize, Deserialize)]
#[register_component]
struct Twitter;

impl Component for Twitter {
  fn register(builder: ComponentRegisterer<Self>) -> ComponentRegisterer<Self>
  where
    Self: Sized,
  {
    builder.handle_read(|_, msg: MsgFoo, _, access| {
      access.cancel();
      msg
    })
  }
}

#[derive(Serialize, Deserialize)]
#[register_component]
struct Panicker;

impl Component for Panicker {
  fn register(builder: ComponentRegisterer<Self>) -> ComponentRegisterer<Self>
  where
    Self: Sized,
  {
    builder.handle_read(|_, _: MsgFoo, _, _| panic!("Panicker got MsgFoo"))
  }
}

#[derive(Message)]
struct MsgFoo;

#[test]
fn cancel() {
  let mut world = World::new();

  let e = world.spawn().with(Twitter).with(Panicker).build();
  world.dispatch(e, MsgFoo);
  // shouldn't panic!
}

#[test]
#[should_panic = "Panicker got MsgFoo"]
fn uncancelled() {
  let mut world = World::new();

  let e = world.spawn_1(Panicker);
  world.dispatch(e, MsgFoo);
}
