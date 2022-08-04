use palkia::prelude::*;

#[test]
fn defer_message() {
    let mut world = World::new();
    world.register_component::<YakShaver>();

    let shaver = world.spawn_1(YakShaver::new(true));

    world.dispatch(shaver, MsgShaveYak::new(16));

    let shave_count = world.query::<&YakShaver>(shaver).unwrap();
    assert_eq!(shave_count.yaks_shaved, 16);
}

#[test]
#[should_panic = "loop of events"]
fn double_borrow_message() {
    let mut world = World::new();
    world.register_component::<YakShaver>();

    let shaver = world.spawn_1(YakShaver::new(false));

    world.dispatch(shaver, MsgShaveYak::new(2));
}

struct YakShaver {
    yaks_shaved: usize,
    defer: bool,
}

impl YakShaver {
    fn new(defer: bool) -> Self {
        Self {
            yaks_shaved: 0,
            defer,
        }
    }
}

impl Component for YakShaver {
    fn register_handlers(builder: HandlerBuilder<Self>) -> HandlerBuilder<Self>
    where
        Self: Sized,
    {
        builder.handle_write(|this, mut msg: MsgShaveYak, e, access| {
            msg.shaves -= 1;
            this.yaks_shaved += 1;

            if msg.shaves > 0 {
                // Doing a naive non-queue here would double-borrow this component.
                if this.defer {
                    access.queue_dispatch(e, msg.clone());
                } else {
                    access.dispatch(e, msg.clone());
                }
            }

            msg
        })
    }
}

#[derive(Clone)]
struct MsgShaveYak {
    pub shaves: usize,
}

impl MsgShaveYak {
    fn new(shaves_left: usize) -> Self {
        Self {
            shaves: shaves_left,
        }
    }
}

impl Message for MsgShaveYak {}
