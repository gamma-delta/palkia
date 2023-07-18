use palkia::prelude::*;

#[test]
#[should_panic(expected = "but that type was not registered")]
fn fail_to_register() {
  let mut world = World::new();

  world.spawn().with(FooBar).build();
}

struct FooBar;

impl Component for FooBar {
  fn register_handlers(builder: HandlerBuilder<Self>) -> HandlerBuilder<Self>
  where
    Self: Sized,
  {
    builder
  }
}
