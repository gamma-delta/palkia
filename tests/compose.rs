use palkia::prelude::*;
use serde::{Deserialize, Serialize};

#[test]
fn compose() {
  let mut world = World::new();

  for id in 0..1000 {
    world.spawn_1(IdHaver(id));
  }

  for e in world.entities() {
    let (idx, gen) = e.decompose();
    let recomp = Entity::recompose(idx, gen);

    let original_id = world.query::<&IdHaver>(e).unwrap().0;
    let recomp_id = world.query::<&IdHaver>(recomp).unwrap().0;

    assert_eq!(original_id, recomp_id);
  }
}

#[derive(Serialize, Deserialize)]
#[register_component(marker)]
struct IdHaver(u32);
