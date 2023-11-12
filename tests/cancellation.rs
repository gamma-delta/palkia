use palkia::prelude::*;

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

struct Panicker;
impl Component for Panicker {
  fn register(builder: ComponentRegisterer<Self>) -> ComponentRegisterer<Self>
  where
    Self: Sized,
  {
    builder.handle_read(|_, _: MsgFoo, _, _| panic!("Panicker got MsgFoo"))
  }
}

struct MsgFoo;
impl Message for MsgFoo {}

#[test]
fn cancel() {
  let mut world = World::new();
  world.register_component::<Twitter>();
  world.register_component::<Panicker>();

  let e = world.spawn().with(Twitter).with(Panicker).build();
  world.dispatch(e, MsgFoo);
  // shouldn't panic!
}

#[test]
#[should_panic = "Panicker got MsgFoo"]
fn uncancelled() {
  let mut world = World::new();
  world.register_component::<Panicker>();

  let e = world.spawn_1(Panicker);
  world.dispatch(e, MsgFoo);
}
