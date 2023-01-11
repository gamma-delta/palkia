use palkia::prelude::*;

#[test]
fn lazy_spawn() {
    let mut world = World::new();

    world.register_component::<Foo>();
    world.register_component::<Bar>();
    world.register_component::<Baz>();

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
    world.register_component::<Foo>();

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

macro_rules! impl_component {
    (@ $ty:ty) => {
        impl Component for $ty {
            fn register_handlers(
                builder: HandlerBuilder<Self>,
            ) -> HandlerBuilder<Self>
            where
                Self: Sized,
            {
                builder
            }
        }
    };
    ($($ty:ty),* $(,)?) => {
        $(
            impl_component!{@ $ty}
        )*
    };
}

struct Foo;
struct Bar;
struct Baz;
impl_component!(Foo, Bar, Baz);
