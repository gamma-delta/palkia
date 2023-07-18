use external::{MsgReproduceMitosis, Rabbit};
use palkia::prelude::*;

/// Imagine this is some external crate.
mod external {
  use palkia::prelude::*;

  pub struct Rabbit;

  impl Component for Rabbit {
    fn register_handlers(builder: HandlerBuilder<Self>) -> HandlerBuilder<Self>
    where
      Self: Sized,
    {
      builder.handle_read(|_, msg: MsgReproduceMitosis, _, access| {
        access.lazy_spawn().with(Rabbit).build();
        msg
      })
    }
  }

  #[derive(Debug, Clone, Copy)]
  pub struct MsgReproduceMitosis;
  impl Message for MsgReproduceMitosis {}
}

/// But now suppose we want to have rabbits that split into 3.
/// Normally we can't, because we can't mess with its impl of Component...
#[derive(Debug, Clone, Copy)]
struct MsgReproduceThree;
impl Message for MsgReproduceThree {}

fn on_three(
  _: &Rabbit,
  msg: MsgReproduceThree,
  _: Entity,
  access: &ListenerWorldAccess,
) -> MsgReproduceThree {
  for _ in 0..2 {
    access.lazy_spawn().with(Rabbit).build();
  }
  msg
}

#[test]
fn extend() {
  let mut world = World::new();
  world.register_component::<Rabbit>();

  // ... but we can extend it here!
  world.extend_component::<Rabbit>(|builder| builder.handle_read(on_three));

  world.spawn_1(Rabbit);

  for _ in 0..5 {
    world.dispatch_to_all(MsgReproduceMitosis);
    world.finalize();
    world.dispatch_to_all(MsgReproduceThree);
    world.finalize();
  }

  assert_eq!(world.len(), (2usize * 3).pow(5))
}
