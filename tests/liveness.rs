use palkia::prelude::*;

#[test]
fn livenesses() {
  let mut world = World::new();

  for _ in 0..100 {
    let e = world.spawn_empty();
    assert_eq!(world.liveness(e), EntityLiveness::Alive);

    world.despawn(e);
    assert_eq!(world.liveness(e), EntityLiveness::Dead);
  }
}

#[test]
fn lazy_livenesses() {
  let mut world = World::new();

  for _ in 0..100 {
    let e = world.lazy_spawn().build();
    assert_eq!(world.liveness(e), EntityLiveness::PartiallySpawned);

    world.finalize();
    assert_eq!(world.liveness(e), EntityLiveness::Alive);

    world.lazy_despawn(e);
    assert_eq!(world.liveness(e), EntityLiveness::Alive);

    world.finalize();
    assert_eq!(world.liveness(e), EntityLiveness::Dead);
  }
}
