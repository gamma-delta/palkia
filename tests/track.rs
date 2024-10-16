pub use palkia::{prelude::*, util::TrackEntitiesWithComponent};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Resource, Default)]
struct ResWidgetTracker(pub TrackEntitiesWithComponent<Widget>);

#[derive(Serialize, Deserialize)]
#[register_component]
struct Widget;

#[derive(Serialize, Deserialize)]
#[register_component(marker)]
struct NotAWidget;

impl Component for Widget {
  fn register(builder: ComponentRegisterer<Self>) -> ComponentRegisterer<Self>
  where
    Self: Sized,
  {
    builder
      .register_create_callback(|_, e, access| {
        let mut wtrack = access.write_resource::<ResWidgetTracker>().unwrap();
        wtrack.0.on_create(e);
      })
      .register_remove_callback(|_, e, access| {
        let mut wtrack = access.write_resource::<ResWidgetTracker>().unwrap();
        wtrack.0.on_remove(e);
      })
  }
}

#[test]
fn test() {
  let mut world = World::new();
  world.insert_resource(ResWidgetTracker::default());

  for _ in 0..50 {
    world.spawn().with(Widget).build();
    world.spawn().with(Widget).with(NotAWidget).build();
    world.spawn().with(NotAWidget).build();
  }
  world.finalize();
  assert_eq!(world.len(), 150);

  {
    let wtracker = world.read_resource::<ResWidgetTracker>().unwrap();
    assert_eq!(wtracker.0.iter().count(), 100);
  }

  for e in world.entities() {
    if world.query::<&NotAWidget>(e).is_some() {
      world.lazy_despawn(e);
    }
  }
  world.finalize();

  {
    let wtracker = world.read_resource::<ResWidgetTracker>().unwrap();
    assert_eq!(wtracker.0.iter().count(), 50);
  }
}
