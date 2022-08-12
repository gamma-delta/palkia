use palkia::prelude::*;
use ron::{ser::PrettyConfig, Options};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
enum ComponentKey {
    Counter,
    Duplicator,
}

struct SerdeInstrs;

impl WorldSerdeInstructions<ComponentKey> for SerdeInstrs {
    fn serialize_entity<S: serde::Serializer>(
        &self,
        mut ctx: EntitySerContext<'_, '_, ComponentKey, S>,
    ) -> Result<(), S::Error> {
        ctx.try_serialize::<Counter>(ComponentKey::Counter)?;
        ctx.try_serialize::<Duplicator>(ComponentKey::Duplicator)?;

        Ok(())
    }

    fn component_count(&self, e: Entity, world: &World) -> Option<usize> {
        // I'm not sure of a less horrible way to do this
        let count = world.query::<&Counter>(e).is_some() as usize
            + world.query::<&Duplicator>(e).is_some() as usize;

        Some(count)
    }

    fn deserialize_entity<'a, 'de, M: serde::de::MapAccess<'de>>(
        &'a self,
        ctx: &mut EntityDeContext<'_, 'de, M, ComponentKey>,
    ) -> Result<(), M::Error>
    where
        'de: 'a,
    {
        match ctx.key() {
            ComponentKey::Counter => ctx.accept::<Counter>(),
            ComponentKey::Duplicator => ctx.accept::<Duplicator>(),
        }
    }
}

#[test]
fn entity_roundtrip() {
    let mut world1 = World::new();
    world1.register_component::<Counter>();
    world1.register_component::<Duplicator>();

    world1.spawn().with(Counter::default()).build();
    world1.spawn().with(Duplicator).build();
    world1
        .spawn()
        .with(Counter::default())
        .with(Duplicator)
        .build();

    for _ in 0..3 {
        world1.dispatch_to_all(MsgTick);
        world1.finalize();
    }

    let entities = world1.iter().collect::<Vec<_>>();

    let ronstr = {
        let mut writer = Vec::new();
        let mut serializer = ron::Serializer::with_options(
            &mut writer,
            Some(PrettyConfig::default()),
            Options::default(),
        )
        .unwrap();
        world1.serialize(SerdeInstrs, &mut serializer).unwrap();
        String::from_utf8(writer).unwrap()
    };

    let mut world2 = World::new();
    world2.register_component::<Counter>();
    world2.register_component::<Duplicator>();

    {
        let mut deserializer = ron::Deserializer::from_str(&ronstr).unwrap();
        world2.deserialize(SerdeInstrs, &mut deserializer).unwrap();
    }

    for e in entities {
        if let Some(w1_counter) = world1.query::<&Counter>(e) {
            let w2_counter = world2.query::<&Counter>(e).unwrap_or_else(|| {
                panic!("{:?} had a counter before serialization but not after", e)
            });
            assert_eq!(
                w1_counter.as_ref(),
                w2_counter.as_ref(),
                "{:?}'s values of counter mismatched",
                e
            );
        }
        if world1.query::<&Duplicator>(e).is_some() {
            assert!(
                world2.query::<&Duplicator>(e).is_some(),
                "{:?} had a duplicator before serialization but not after",
                e
            )
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default, PartialEq, Eq)]
struct Counter {
    count: u64,
}

impl Component for Counter {
    fn register_handlers(builder: HandlerBuilder<Self>) -> HandlerBuilder<Self>
    where
        Self: Sized,
    {
        builder.handle_write(|this, msg: MsgTick, _, _| {
            this.count += 1;
            msg
        })
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct Duplicator;

impl Component for Duplicator {
    fn register_handlers(builder: HandlerBuilder<Self>) -> HandlerBuilder<Self>
    where
        Self: Sized,
    {
        builder.handle_read(|_, msg: MsgTick, _, access| {
            access.lazy_spawn().with(Duplicator).build();
            msg
        })
    }
}

#[derive(Clone)]
struct MsgTick;
impl Message for MsgTick {}
