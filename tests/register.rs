use palkia::prelude::*;
use serde::{Deserialize, Serialize};

#[test]
#[should_panic(expected = "without registering it")]
fn fail_to_register() {
  let mut world = World::new();

  world.spawn().with(FooBar).build();
}

// note lack of attr or macro call here
#[derive(Serialize, Deserialize)]
struct FooBar;

impl Component for FooBar {
  fn register(builder: ComponentRegisterer<Self>) -> ComponentRegisterer<Self>
  where
    Self: Sized,
  {
    builder
  }
}
