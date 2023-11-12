#![cfg(feature = "serde")]

use palkia::{prelude::*, serde::*};
use serde::{Deserialize, Serialize};

#[test]
fn entity_roundtrip() {
  let mut world1 = registered_world();

  world1.spawn().with(Counter::default()).build();
  world1.spawn().with(Duplicator).build();
  world1
    .spawn()
    .with(Counter::default())
    .with(Duplicator)
    .build();

  for _ in 0..10 {
    world1.dispatch_to_all(MsgTick);
    world1.finalize();
  }

  let entities = world1.iter().collect::<Vec<_>>();

  let bin = ser_world(&mut world1);

  let mut world2 = registered_world();

  de_world(&mut world2, &bin);

  assert_eq!(world1.len(), world2.len());
  for e in entities {
    assert_eq!(
      world1.len_of(e),
      world2.len_of(e),
      "{:?} had mismatching component counts",
      e
    );
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

#[test]
fn resource_roundtrip() {
  let mut world1 = World::new();
  world1.insert_resource(AResource {
    foo: 7604,
    bar: 69420,
  });
  world1.insert_resource(ResNotSerialized);

  let bin = ser_world(&mut world1);

  let mut world2 = World::new();
  de_world(&mut world2, &bin);

  let a_res1 = world1.get_resource::<AResource>().unwrap();
  let a_res2 = world2.get_resource::<AResource>().unwrap();
  assert_eq!(a_res1, a_res2);

  assert!(!world2.contains_resource::<ResNotSerialized>());
}

#[test]
fn roundtrip_all() {
  let mut world1 = registered_world();

  world1.insert_resource(AResource {
    foo: 0xF00,
    bar: 0xBA2,
  });
  world1.insert_resource(DupliCounter::default());

  world1.spawn().with(Counter::default()).build();
  world1.spawn().with(Duplicator).build();
  world1
    .spawn()
    .with(Counter::default())
    .with(Duplicator)
    .build();

  for _ in 0..10 {
    world1.dispatch_to_all(MsgTick);
    world1.finalize();
  }

  let entities = world1.iter().collect::<Vec<_>>();

  let bin = ser_world(&mut world1);

  let mut world2 = registered_world();

  de_world(&mut world2, &bin);

  assert_eq!(world1.len(), world2.len());
  for e in entities {
    assert_eq!(
      world1.len_of(e),
      world2.len_of(e),
      "{:?} had mismatching component counts",
      e
    );

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

  let a_res1 = world1.get_resource::<AResource>().unwrap();
  let a_res2 = world2.get_resource::<AResource>().unwrap();
  assert_eq!(a_res1, a_res2);

  let dc1 = world1.get_resource::<DupliCounter>().unwrap();
  let dc2 = world2.get_resource::<DupliCounter>().unwrap();
  assert_eq!(dc1, dc2);
}

#[test]
fn callbacks() {
  let mut world1 = registered_world();
  world1.insert_resource_default::<ResThatIncrementsANumberWhenADuplicatorIsCreated>();

  world1.spawn().with(Duplicator).build();

  for _ in 0..10 {
    world1.dispatch_to_all(MsgTick);
    world1.finalize();
  }

  let bin = ser_world(&mut world1);

  let mut world2 = registered_world();
  world2.insert_resource_default::<ResThatIncrementsANumberWhenADuplicatorIsCreated>();

  de_world(&mut world2, &bin);

  let rtianwadic1 = world1
    .get_resource::<ResThatIncrementsANumberWhenADuplicatorIsCreated>()
    .unwrap();
  let rtianwadic2 = world2
    .get_resource::<ResThatIncrementsANumberWhenADuplicatorIsCreated>()
    .unwrap();
  // note that we don't actually de/serialize that resource
  assert_eq!(rtianwadic1.count, rtianwadic2.count);
}

// Serde helpers

fn ser_world(world: &mut World) -> Vec<u8> {
  let mut writer = Vec::new();
  let mut serializer =
    bincode::Serializer::new(&mut writer, bincode::DefaultOptions::new());
  world.serialize(SerdeInstrs, &mut serializer).unwrap();
  writer
}

fn de_world(world: &mut World, bin: &[u8]) {
  let mut deserializer =
    bincode::Deserializer::from_slice(&bin, bincode::DefaultOptions::new());
  world.deserialize(SerdeInstrs, &mut deserializer).unwrap();
}

// World serde impl

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
enum ResourceKey {
  AResource,
  DupliCounter,
}

struct SerdeInstrs;

impl WorldSerdeInstructions<ResourceKey> for SerdeInstrs {
  fn serialize_entity<S: serde::Serializer>(
    &self,
    mut ctx: EntitySerContext<'_, '_, S>,
  ) -> Result<(), S::Error> {
    ctx.try_serialize::<Counter>()?;
    ctx.try_serialize::<Duplicator>()?;

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
    ctx: &mut EntityDeContext<'_, 'de, M>,
  ) -> Result<(), M::Error>
  where
    'de: 'a,
  {
    match ctx.key() {
      "Counter" => ctx.accept::<Counter>(),
      "Duplicator" => ctx.accept::<Duplicator>(),
      _ => panic!(),
    }
  }

  fn serialize_resource<S: serde::Serializer>(
    &self,
    mut ctx: ResourceSerContext<'_, '_, ResourceKey, S>,
  ) -> Result<(), S::Error> {
    ctx.try_serialize::<AResource>(ResourceKey::AResource)?;
    ctx.try_serialize::<DupliCounter>(ResourceKey::DupliCounter)?;

    Ok(())
  }

  fn resource_count(&self, world: &World) -> Option<usize> {
    let count = world.contains_resource::<AResource>() as usize
      + world.contains_resource::<DupliCounter>() as usize;

    Some(count)
  }

  fn deserialize_resource<'a, 'de, M: serde::de::MapAccess<'de>>(
    &'a self,
    ctx: &mut ResourceDeContext<'_, 'de, M, ResourceKey>,
  ) -> Result<(), M::Error>
  where
    'de: 'a,
  {
    match ctx.key() {
      ResourceKey::AResource => ctx.accept::<AResource>(),
      ResourceKey::DupliCounter => ctx.accept::<DupliCounter>(),
    }
  }
}

// ECM stuff

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
    builder
      .handle_read(|_, msg: MsgTick, _, access| {
        access.lazy_spawn().with(Duplicator).build();

        if let Ok(mut duplicounter) = access.write_resource::<DupliCounter>() {
          duplicounter.count += 1;
        }

        msg
      })
      .register_remove_callback(|_, _, access| {
        if let Ok(mut rtianwadic) =
          access
            .write_resource::<ResThatIncrementsANumberWhenADuplicatorIsCreated>(
            )
        {
          rtianwadic.count += 1;
        }
      })
  }
}

fn registered_world() -> World {
  let mut w = World::new();
  w.register_component::<Counter>();
  w.register_component::<Duplicator>();
  w
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct AResource {
  foo: i32,
  bar: i32,
}
impl Resource for AResource {}

#[derive(Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct DupliCounter {
  count: u64,
}
impl Resource for DupliCounter {}

struct ResNotSerialized;
impl Resource for ResNotSerialized {}

/// Basically the same as DupliCounter but it does it with a callback
/// and isn't serialized
#[derive(Default)]
struct ResThatIncrementsANumberWhenADuplicatorIsCreated {
  count: u64,
}
impl Resource for ResThatIncrementsANumberWhenADuplicatorIsCreated {}

#[derive(Clone)]
struct MsgTick;
impl Message for MsgTick {}
