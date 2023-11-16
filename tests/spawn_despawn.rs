//! Check lazy spawning and despawning is sound.

use palkia::prelude::*;
use serde::{Deserialize, Serialize};

#[test]
fn spawn() {
  let mut world = World::new();

  world.spawn().with(Rabbit).build();

  for _ in 0..16 {
    world.dispatch_to_all(MsgReproduceMitosis);
    world.finalize();
  }

  // Each generation the population doubles
  assert_eq!(world.len(), 2usize.pow(16));
}

#[test]
fn spawn_despawn() {
  let mut world = World::new();

  world.spawn().with(Rabbit).build();

  for _ in 0..16 {
    world.dispatch_to_all(MsgReproduceAndDie);
    world.finalize();
  }

  // Each generation the population still doubles!
  assert_eq!(world.len(), 2usize.pow(16))
}

#[test]
fn spawn_dedespawn() {
  let mut world = World::new();

  world.spawn().with(Rabbit).build();

  for _ in 0..100 {
    world.dispatch_to_all(MsgReproduceAndDieAndDie);
    world.finalize();
  }

  assert_eq!(world.len(), 1)
}

#[test]
fn spawn_again() {
  let mut world = World::new();

  let mut builder = world.spawn();

  let builder2 = builder.spawn_again();
  builder2.with(Rabbit).build();

  builder.with(Rabbit).build();

  assert_eq!(world.len(), 2);
}

#[test]
fn infanticide() {
  let mut world = World::new();

  world.spawn_1(Rabbit);

  for _ in 0..100 {
    world.dispatch_to_all(MsgReproduceButThenJustKillYourOffspring);
    world.finalize();
    assert_eq!(world.len(), 1);
  }
}

#[derive(Serialize, Deserialize)]
#[register_component]
struct Rabbit;

impl Rabbit {
  /// Every rabbit duplicates itself.
  fn mitosis(
    &self,
    event: MsgReproduceMitosis,
    _: Entity,
    access: &ListenerWorldAccess,
  ) -> MsgReproduceMitosis {
    access.lazy_spawn().with(Rabbit).build();

    event
  }

  fn reproduce_and_die(
    &self,
    event: MsgReproduceAndDie,
    this: Entity,
    access: &ListenerWorldAccess,
  ) -> MsgReproduceAndDie {
    // Make sure that interleaving birth and death works
    access.lazy_spawn().with(Rabbit).build();
    access.lazy_despawn(this);
    access.lazy_spawn().with(Rabbit).build();

    event
  }

  fn reproduce_and_die_and_die(
    &self,
    event: MsgReproduceAndDieAndDie,
    this: Entity,
    access: &ListenerWorldAccess,
  ) -> MsgReproduceAndDieAndDie {
    // Make sure killing twice isn't a problem
    access.lazy_spawn().with(Rabbit).build();
    access.lazy_despawn(this);
    access.lazy_despawn(this);

    event
  }

  fn reproduce_and_infanticide(
    &self,
    event: MsgReproduceButThenJustKillYourOffspring,
    _this: Entity,
    access: &ListenerWorldAccess,
  ) -> MsgReproduceButThenJustKillYourOffspring {
    // Make sure killing twice isn't a problem
    let kiddo = access.lazy_spawn().with(Rabbit).build();
    access.lazy_despawn(kiddo);

    event
  }
}

impl Component for Rabbit {
  fn register(builder: ComponentRegisterer<Self>) -> ComponentRegisterer<Self>
  where
    Self: Sized,
  {
    builder
      .handle_read(Rabbit::mitosis)
      .handle_read(Rabbit::reproduce_and_die)
      .handle_read(Rabbit::reproduce_and_die_and_die)
      .handle_read(Rabbit::reproduce_and_infanticide)
  }
}

#[derive(Message, Debug, Clone, Copy)]
struct MsgReproduceMitosis;

#[derive(Message, Debug, Clone, Copy)]
struct MsgReproduceAndDie;

#[derive(Message, Debug, Clone, Copy)]
struct MsgReproduceAndDieAndDie;

#[derive(Message, Debug, Clone, Copy)]
struct MsgReproduceButThenJustKillYourOffspring;
