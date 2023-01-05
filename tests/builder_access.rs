use palkia::prelude::*;

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

struct Foo(u32);
struct Bar(String);
struct Baz(i32);
impl_component!(Foo, Bar, Baz);

#[test]
fn get_components_off_builder() {
    let mut world = World::new();
    world.register_component::<Foo>();
    world.register_component::<Bar>();
    world.register_component::<Baz>();

    let mut builder = world.spawn();
    builder.insert(Foo(42));
    builder.insert(Bar("Hello, world!".to_string()));

    let mut foo = builder.get_component_mut::<Foo>().unwrap();
    assert_eq!(foo.0, 42);
    foo.0 = 7604;

    builder.insert(Baz(-69));
    builder.insert(Bar("Elbereth".to_string()));
    let baz = builder.get_component::<Baz>().unwrap();
    assert_eq!(baz.0, -69);

    let built = builder.build();

    let (foo, bar, baz) = world.query::<(&Foo, &Bar, &Baz)>(built).unwrap();
    assert_eq!(foo.0, 7604);
    assert_eq!(bar.0.as_str(), "Elbereth");
    assert_eq!(baz.0, -69);
}
