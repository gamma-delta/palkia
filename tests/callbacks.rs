//! Check that callbacks work.

use palkia::prelude::*;
use serde::{Deserialize, Serialize};

#[test]
fn create() {
  let mut world = World::new();

  world.insert_resource(PopulationTracker(0));

  world.spawn().with(Rabbit).build();
  {
    let pop = world.get_resource::<PopulationTracker>().unwrap();
    assert_eq!(pop.0, 1);
  }

  for _ in 0..100 {
    world.spawn_1(NotRabbit);
  }

  for i in 0..16 {
    world.dispatch_to_all(MsgReproduceMitosis);
    world.finalize();

    // Each generation the population doubles
    let pop = world.get_resource::<PopulationTracker>().unwrap();
    assert_eq!(pop.0, 2u64.pow(i + 1));
  }
}

#[test]
fn create_remove() {
  let mut world = World::new();

  world.insert_resource(PopulationTracker(0));

  world.spawn().with(Rabbit).build();
  {
    let pop = world.get_resource::<PopulationTracker>().unwrap();
    assert_eq!(pop.0, 1);
  }

  for _ in 0..100 {
    world.spawn_1(NotRabbit);
  }

  for i in 0..16 {
    world.dispatch_to_all(MsgReproduceAndDie);
    world.finalize();

    // Each generation the population still doubles
    let pop = world.get_resource::<PopulationTracker>().unwrap();
    assert_eq!(pop.0, 2u64.pow(i + 1));
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
}

impl Component for Rabbit {
  fn register(builder: ComponentRegisterer<Self>) -> ComponentRegisterer<Self>
  where
    Self: Sized,
  {
    builder
      .handle_read(Rabbit::mitosis)
      .handle_read(Rabbit::reproduce_and_die)
      .register_create_callback(|_, _, access| {
        let mut population =
          access.write_resource::<PopulationTracker>().unwrap();
        population.0 += 1;
      })
      .register_remove_callback(|_, _, access| {
        let mut population =
          access.write_resource::<PopulationTracker>().unwrap();
        population.0 -= 1;
      })
  }
}

// struct to make sure it works with other components in there
#[derive(Serialize, Deserialize)]
#[register_component]
struct NotRabbit;

impl Component for NotRabbit {
  fn register(builder: ComponentRegisterer<Self>) -> ComponentRegisterer<Self>
  where
    Self: Sized,
  {
    builder
  }
}

#[derive(Resource, Serialize, Deserialize)]
struct PopulationTracker(u64);

#[derive(Message, Debug, Clone, Copy)]
struct MsgReproduceMitosis;

#[derive(Message, Debug, Clone, Copy)]
struct MsgReproduceAndDie;
