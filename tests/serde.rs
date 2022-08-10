use palkia::prelude::*;
use serde::{Deserialize, Serialize};

// this test doesn't pass *yet* because of problems with leaking entities
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
        world.finalize();
    }

    let ronstr = {
        let mut writer = Vec::new();
        let mut serializer =
            ron::ser::Serializer::with_options(&mut writer, None, ron::Options::default()).unwrap();
        world
            .serialize_entities::<_, &'static str>(&mut serializer)
            .unwrap();

        // Roundtrip thru ron to make sure the map is sorted
        let ronval = ron::de::from_bytes::<ron::Value>(&writer).unwrap();
        ron::ser::to_string(&ronval).unwrap()
    };

    let expect = r#"
{
    {"generation":0,"index":0}: { "counter": {"count": 3} },
    {"generation":0,"index":1}: { "duplicator": () },
    {"generation":0,"index":2}: { "counter": {"count": 3}, "duplicator": () },

    {"generation":0,"index":3}: { "duplicator": () },
    {"generation":0,"index":4}: { "duplicator": () },
    {"generation":0,"index":5}: { "duplicator": () },
    {"generation":0,"index":6}: { "duplicator": () },
    {"generation":0,"index":7}: { "duplicator": () },
    {"generation":0,"index":8}: { "duplicator": () },
    {"generation":0,"index":9}: { "duplicator": () },
    {"generation":0,"index":10}: { "duplicator": () },
    {"generation":0,"index":11}: { "duplicator": () },
    {"generation":0,"index":12}: { "duplicator": () },
    {"generation":0,"index":13}: { "duplicator": () },
    {"generation":0,"index":14}: { "duplicator": () },
    {"generation":0,"index":15}: { "duplicator": () },
    {"generation":0,"index":16}: { "duplicator": () }
}    
"#
    .replace(char::is_whitespace, "");
    assert_eq!(ronstr.as_str(), expect.as_str());
}

#[derive(Serialize, Deserialize, Default, PartialEq, Eq)]
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
            .handle_read(Counter::serde_handler)
    }
}

impl SerDeComponent<&'static str> for Counter {
    fn get_id() -> &'static str {
        "counter"
    }
}

#[derive(Serialize, Deserialize, PartialEq, Eq)]
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
            .handle_read(Duplicator::serde_handler)
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
