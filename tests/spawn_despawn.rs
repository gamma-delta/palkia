//! Check lazy spawning and despawning is sound.

use palkia::prelude::*;

#[test]
fn spawn() {
    let mut world = World::new();

    world.register_component::<Rabbit>();

    world.spawn().with(Rabbit::new()).build();

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

    world.spawn().with(Rabbit::new()).build();

    for _ in 0..16 {
        world.dispatch_to_all(MsgReproduceAndDie);
        world.finalize();
    }

    // Each generation the population still doubles!
    assert_eq!(world.len(), 2usize.pow(16))
}

struct Rabbit {
    generation: u32,
}

impl Rabbit {
    fn new() -> Self {
        Self { generation: 0 }
    }

    fn offspring(&self) -> Self {
        Self {
            generation: self.generation + 1,
        }
    }

    /// Every rabbit duplicates itself.
    fn mitosis(
        &mut self,
        event: MsgReproduceMitosis,
        _: Entity,
        access: &WorldAccess,
    ) -> MsgReproduceMitosis {
        access.lazy_spawn().with(self.offspring()).build();

        event
    }

    fn reproduce_and_die(
        &mut self,
        event: MsgReproduceAndDie,
        this: Entity,
        access: &WorldAccess,
    ) -> MsgReproduceAndDie {
        // Make sure that interleaving birth and death works
        access.lazy_spawn().with(self.offspring()).build();
        access.lazy_despawn(this);
        access.lazy_spawn().with(self.offspring()).build();

        event
    }
}

impl Component for Rabbit {
    fn register_handlers(builder: HandlerBuilder<Self>) -> HandlerBuilder<Self>
    where
        Self: Sized,
    {
        builder
            .handle_write(Rabbit::mitosis)
            .handle_write(Rabbit::reproduce_and_die)
    }

    fn priority() -> u64
    where
        Self: Sized,
    {
        0
    }
}

#[derive(Debug, Clone, Copy)]
struct MsgReproduceMitosis;

impl Message for MsgReproduceMitosis {}

#[derive(Debug, Clone, Copy)]
struct MsgReproduceAndDie;

impl Message for MsgReproduceAndDie {}
