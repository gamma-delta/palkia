use palkia::prelude::*;

#[test]
fn query() {
    let mut world = World::new();
    world.register_component::<Foo>();
    world.register_component::<Bar>();
    world.register_component::<Baz>();

    let foo = world.spawn().with(Foo).build();
    let bar = world.spawn().with(Bar).build();
    let foobar = world.spawn().with(Foo).with(Bar).build();
    let foobaz = world.spawn().with(Foo).with(Baz).build();
    let empty = world.spawn_empty();

    assert!(world.query::<&Foo>(foo).is_some());
    assert!(world.query::<&Bar>(bar).is_some());
    assert!(world.query::<&Baz>(foobaz).is_some());

    assert!(world.query::<(&Foo, &Bar)>(foobar).is_some());
    assert!(world.query::<(&Foo, &Baz)>(foobaz).is_some());

    assert!(world.query::<&Foo>(bar).is_none());
    assert!(world.query::<(&Foo, &Bar)>(foo).is_none());

    let q = world.query::<(Option<&Foo>, &Bar)>(foobar).unwrap();
    assert!(q.0.is_some());

    let q = world.query::<(Option<&Foo>, &Bar)>(foobaz);
    assert!(q.is_none());

    let q = world.query::<Option<(&Foo, &Baz)>>(foobaz).unwrap();
    assert!(q.is_some());

    // needs to fetch *both*
    let q = world.query::<Option<(&Foo, &Baz)>>(foobar).unwrap();
    assert!(q.is_none());

    assert!(world.query::<Option<(&Foo, &Bar, &Baz)>>(empty).is_some());
}

#[test]
fn double_query() {
    let mut world = World::new();
    world.register_component::<Foo>();
    world.register_component::<Bar>();

    let foo = world.spawn().with(Foo).build();
    let foobar = world.spawn().with(Foo).with(Bar).build();

    {
        let _q1 = world.query::<&Foo>(foo).unwrap();
        let _q2 = world.query::<&Foo>(foo).unwrap();
    }

    {
        let _q1 = world.query::<&mut Foo>(foobar).unwrap();
        let _q2 = world.query::<&mut Bar>(foobar).unwrap();
    }
}

#[test]
#[should_panic(
    expected = "Entity { index: 0, generation: 0 } had a component write queried when it was borrowed"
)]
fn double_query_rw() {
    let mut world = World::new();
    world.register_component::<Foo>();

    let foo = world.spawn().with(Foo).build();

    let _q = world.query::<&Foo>(foo).unwrap();
    // should panic here
    let _q2 = world.query::<&mut Foo>(foo).unwrap();
}

#[test]
#[should_panic(
    expected = "Entity { index: 0, generation: 0 } had a component read queried when it was mutably"
)]
fn double_query_wr() {
    let mut world = World::new();
    world.register_component::<Foo>();

    let foo = world.spawn().with(Foo).build();

    let _q = world.query::<&mut Foo>(foo).unwrap();
    // should panic here
    let _q2 = world.query::<&Foo>(foo).unwrap();
}
#[test]
#[should_panic(
    expected = "Entity { index: 0, generation: 0 } had a component write queried when it was borrowed"
)]
fn double_query_ww() {
    let mut world = World::new();
    world.register_component::<Foo>();

    let foo = world.spawn().with(Foo).build();

    let _q = world.query::<&mut Foo>(foo).unwrap();
    // should panic here
    let _q2 = world.query::<&mut Foo>(foo).unwrap();
}

#[derive(Debug, PartialEq, Eq)]
struct Foo;

impl Component for Foo {
    fn register_handlers(builder: HandlerBuilder<Self>) -> HandlerBuilder<Self>
    where
        Self: Sized,
    {
        builder
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Bar;

impl Component for Bar {
    fn register_handlers(builder: HandlerBuilder<Self>) -> HandlerBuilder<Self>
    where
        Self: Sized,
    {
        builder
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Baz;

impl Component for Baz {
    fn register_handlers(builder: HandlerBuilder<Self>) -> HandlerBuilder<Self>
    where
        Self: Sized,
    {
        builder
    }
}
