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
    }
}

#[derive(Debug, Clone, Copy)]
struct MsgReproduceMitosis;

impl Message for MsgReproduceMitosis {}

#[derive(Debug, Clone, Copy)]
struct MsgReproduceAndDie;

impl Message for MsgReproduceAndDie {}
