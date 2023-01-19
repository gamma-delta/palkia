//! Check that callbacks work.

use palkia::prelude::*;

#[test]
fn create() {
  let mut world = World::new();

  world.register_component::<Rabbit>();
  world.register_component::<NotRabbit>();
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

  world.register_component::<Rabbit>();
  world.register_component::<NotRabbit>();
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
  fn register_handlers(builder: HandlerBuilder<Self>) -> HandlerBuilder<Self>
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
struct NotRabbit;
impl Component for NotRabbit {
  fn register_handlers(builder: HandlerBuilder<Self>) -> HandlerBuilder<Self>
  where
    Self: Sized,
  {
    builder
  }
}

struct PopulationTracker(u64);

impl Resource for PopulationTracker {}

#[derive(Debug, Clone, Copy)]
struct MsgReproduceMitosis;

impl Message for MsgReproduceMitosis {}

#[derive(Debug, Clone, Copy)]
struct MsgReproduceAndDie;

impl Message for MsgReproduceAndDie {}
