use palkia::prelude::*;
use serde::{Deserialize, Serialize};

/*
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
    [0,0]: { "counter": {"count": 3} },
    [1,0]: { "duplicator": () },
    [2,0]: { "counter": {"count": 3}, "duplicator": () },
    [3,0]: { "duplicator": () },
    [4,0]: { "duplicator": () },
    [5,0]: { "duplicator": () },
    [6,0]: { "duplicator": () },
    [7,0]: { "duplicator": () },
    [8,0]: { "duplicator": () },
    [9,0]: { "duplicator": () },
    [10,0]: { "duplicator": () },
    [11,0]: { "duplicator": () },
    [12,0]: { "duplicator": () },
    [13,0]: { "duplicator": () },
    [14,0]: { "duplicator": () },
    [15,0]: { "duplicator": () },
    [16,0]: { "duplicator": () }
}
"#
    .replace(char::is_whitespace, "");
    assert_eq!(ronstr.as_str(), expect.as_str());
}
*/

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
            .handle_read(Counter::ser_handler)
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
            .handle_read(Duplicator::ser_handler)
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
