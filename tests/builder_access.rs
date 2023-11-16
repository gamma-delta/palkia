use palkia::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[register_component(marker)]
struct Foo(u32);
#[derive(Serialize, Deserialize)]
#[register_component(marker)]
struct Bar(String);
#[derive(Serialize, Deserialize)]
#[register_component(marker)]
struct Baz(i32);

#[test]
fn get_components_off_builder() {
  let mut world = World::new();

  let mut builder = world.spawn();
  builder.insert(Foo(42));
  builder.insert(Bar("Hello, world!".to_string()));

  let foo = builder.get_component_mut::<Foo>().unwrap();
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
