use palkia::prelude::*;
use serde::{Deserialize, Serialize};

#[test]
fn ser_entities() {
    let mut world = World::new();

    world.register_component::<Counter>();
    world.register_component::<Duplicator>();

    world.spawn().with(Counter::default()).build();
    world.spawn().with(Duplicator).build();
    world
        .spawn()
        .with(Counter::default())
        .with(Duplicator)
        .build();

    for _ in 0..3 {
        world.dispatch_to_all(MsgTick);
    }

    let stringified = {
        let mut writer = Vec::new();
        let mut serializer =
            ron::ser::Serializer::with_options(&mut writer, None, ron::Options::default()).unwrap();
        world
            .serialize_entities::<_, &'static str>(&mut serializer)
            .unwrap();
        String::from_utf8(writer).unwrap()
    };
    panic!("{}", stringified);
}

#[derive(Serialize, Deserialize, Default)]
struct Counter {
    count: u64,
}

impl Component for Counter {
    fn register_handlers(builder: HandlerBuilder<Self>) -> HandlerBuilder<Self>
    where
        Self: Sized,
    {
        builder
            .handle_write(|this, msg: MsgTick, _, _| {
                this.count += 1;
                msg
            })
            .handle_read(<Counter as SerDeComponent<&'static str>>::serde_handler)
    }
}

impl SerDeComponent<&'static str> for Counter {
    fn get_id() -> &'static str {
        "counter"
    }
}

#[derive(Serialize, Deserialize)]
struct Duplicator;

impl Component for Duplicator {
    fn register_handlers(builder: HandlerBuilder<Self>) -> HandlerBuilder<Self>
    where
        Self: Sized,
    {
        builder
            .handle_read(|_, msg: MsgTick, _, access| {
                access.lazy_spawn().with(Duplicator).build();
                msg
            })
            .handle_read(<Duplicator as SerDeComponent<&'static str>>::serde_handler)
    }
}

impl SerDeComponent<&'static str> for Duplicator {
    fn get_id() -> &'static str {
        "duplicator"
    }
}

#[derive(Clone)]
struct MsgTick;
impl Message for MsgTick {}
