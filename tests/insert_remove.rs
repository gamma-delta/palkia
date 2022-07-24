//! Check lazy insertion and removal of components is sound.

use palkia::prelude::*;

#[test]
fn insertion() {
    let mut world = World::new();
    world.register_component::<Incrementer<0>>();
    world.register_component::<Incrementer<1>>();
    world.register_component::<Incrementer<2>>();
    world.register_component::<Incrementer<3>>();
    world.register_component::<Incrementer<4>>();
    world.register_component::<Incrementer<5>>();
    world.register_component::<Incrementer<6>>();
    world.register_component::<Incrementer<7>>();

    let target = world.spawn_1(Incrementer::<0>);

    for _ in 0..7 {
        world.dispatch_to_all(MsgTick);
        world.finalize();
        assert_eq!(world.len_of(target), 1);
    }
}

#[test]
fn deletion() {
    let mut world = World::new();
    world.register_component::<Remover<0>>();
    world.register_component::<Remover<1>>();
    world.register_component::<Remover<2>>();
    world.register_component::<Remover<3>>();
    world.register_component::<Remover<4>>();
    world.register_component::<Remover<5>>();
    world.register_component::<Remover<6>>();
    world.register_component::<Remover<7>>();

    let target = world
        .spawn()
        .with(Remover::<0>)
        .with(Remover::<1>)
        .with(Remover::<2>)
        .with(Remover::<3>)
        .with(Remover::<4>)
        .with(Remover::<5>)
        .with(Remover::<6>)
        .with(Remover::<7>)
        .build();

    world.dispatch_to_all(MsgTick);
    world.finalize();
    assert_eq!(world.len_of(target), 0);
}

#[test]
fn insertion_deletion() {
    let mut world = World::new();
    world.register_component::<Breadcrumber<0>>();
    world.register_component::<Breadcrumber<1>>();
    world.register_component::<Breadcrumber<2>>();
    world.register_component::<Breadcrumber<3>>();
    world.register_component::<Breadcrumber<4>>();
    world.register_component::<Breadcrumber<5>>();
    world.register_component::<Breadcrumber<6>>();
    world.register_component::<Breadcrumber<7>>();

    let target = world.spawn_1(Breadcrumber::<0>);

    for i in 0..7 {
        world.dispatch_to_all(MsgTick);
        world.finalize();
        assert_eq!(world.len_of(target), i + 2);
    }

    assert!(world.query::<&Breadcrumber<7>>(target).is_some());
}

/// Inserts itself plus one.
struct Breadcrumber<const N: usize>;
impl<const N: usize> Component for Breadcrumber<N> {
    fn register_handlers(builder: HandlerBuilder<Self>) -> HandlerBuilder<Self>
    where
        Self: Sized,
    {
        builder.handle_read(|_: &Self, msg: MsgTick, e: Entity, access: &WorldAccess| {
            match N {
                0 => access.lazy_insert_at(e, Breadcrumber::<1>),
                1 => access.lazy_insert_at(e, Breadcrumber::<2>),
                2 => access.lazy_insert_at(e, Breadcrumber::<3>),
                3 => access.lazy_insert_at(e, Breadcrumber::<4>),
                4 => access.lazy_insert_at(e, Breadcrumber::<5>),
                5 => access.lazy_insert_at(e, Breadcrumber::<6>),
                6 => access.lazy_insert_at(e, Breadcrumber::<7>),
                _ => {}
            }

            msg
        })
    }

    fn priority() -> u64
    where
        Self: Sized,
    {
        0 + N as u64
    }
}

/// Deletes itself.
struct Remover<const N: usize>;
impl<const N: usize> Component for Remover<N> {
    fn register_handlers(builder: HandlerBuilder<Self>) -> HandlerBuilder<Self>
    where
        Self: Sized,
    {
        builder.handle_read(|_: &Self, msg: MsgTick, e: Entity, access: &WorldAccess| {
            access.lazy_remove_at::<Self>(e);

            msg
        })
    }

    fn priority() -> u64
    where
        Self: Sized,
    {
        100 + N as u64
    }
}

/// Creates another one of itself up to N=15.
struct Incrementer<const N: usize>;
impl<const N: usize> Component for Incrementer<N> {
    fn register_handlers(builder: HandlerBuilder<Self>) -> HandlerBuilder<Self>
    where
        Self: Sized,
    {
        builder.handle_read(|_: &Self, msg: MsgTick, e: Entity, access: &WorldAccess| {
            match N {
                0 => access.lazy_insert_at(e, Incrementer::<1>),
                1 => access.lazy_insert_at(e, Incrementer::<2>),
                2 => access.lazy_insert_at(e, Incrementer::<3>),
                3 => access.lazy_insert_at(e, Incrementer::<4>),
                4 => access.lazy_insert_at(e, Incrementer::<5>),
                5 => access.lazy_insert_at(e, Incrementer::<6>),
                6 => access.lazy_insert_at(e, Incrementer::<7>),
                _ => {}
            }
            access.lazy_remove_at::<Self>(e);

            msg
        })
    }

    fn priority() -> u64
    where
        Self: Sized,
    {
        200 + N as u64
    }
}

#[derive(Debug, Clone)]
struct MsgTick;
impl Message for MsgTick {}
