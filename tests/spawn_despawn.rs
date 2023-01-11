//! Check lazy spawning and despawning is sound.

use palkia::prelude::*;

#[test]
fn spawn() {
    let mut world = World::new();

    world.register_component::<Rabbit>();

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

    world.register_component::<Rabbit>();

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

    world.register_component::<Rabbit>();

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
    world.register_component::<Rabbit>();

    let mut builder = world.spawn();

    let builder2 = builder.spawn_again();
    builder2.with(Rabbit).build();

    builder.with(Rabbit).build();

    assert_eq!(world.len(), 2);
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
}

impl Component for Rabbit {
    fn register_handlers(builder: HandlerBuilder<Self>) -> HandlerBuilder<Self>
    where
        Self: Sized,
    {
        builder
            .handle_read(Rabbit::mitosis)
            .handle_read(Rabbit::reproduce_and_die)
            .handle_read(Rabbit::reproduce_and_die_and_die)
    }
}

#[derive(Debug, Clone, Copy)]
struct MsgReproduceMitosis;

impl Message for MsgReproduceMitosis {}

#[derive(Debug, Clone, Copy)]
struct MsgReproduceAndDie;

impl Message for MsgReproduceAndDie {}

#[derive(Debug, Clone, Copy)]
struct MsgReproduceAndDieAndDie;

impl Message for MsgReproduceAndDieAndDie {}
