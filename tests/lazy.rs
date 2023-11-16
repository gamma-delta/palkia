use palkia::prelude::*;
use serde::{Deserialize, Serialize};

#[test]
fn lazy_spawn() {
  let mut world = World::new();

  let world_ref = &world;
  let e = world_ref.lazy_spawn().with(Foo).with(Bar).build();
  for _ in 0..100 {
    world_ref.lazy_spawn().with(Baz).build();
    assert_eq!(world_ref.len(), 0);
  }

  world.finalize();
  assert_eq!(world.len(), 101);
  world.query::<(&Foo, &Bar)>(e).unwrap();
}

#[test]
fn lazy_despawn() {
  let mut world = World::new();

  let es = (0..100)
    .map(|i| {
      let e = world.spawn().with(Foo).build();
      assert_eq!(i + 1, world.len());
      e
    })
    .collect::<Vec<_>>();

  let world_ref = &world;
  for e in es {
    world_ref.lazy_despawn(e);
  }

  assert_eq!(world.len(), 100);
  world.finalize();
  assert_eq!(world.len(), 0);
}

#[derive(Serialize, Deserialize)]
#[register_component(marker)]
struct Foo;
#[derive(Serialize, Deserialize)]
#[register_component(marker)]
struct Bar;
#[derive(Serialize, Deserialize)]
#[register_component(marker)]
struct Baz;
