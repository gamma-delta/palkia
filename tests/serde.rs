use palkia::{
  manually_register_resource, prelude::*, resource::ResourceRegisterer,
};
use serde::{Deserialize, Serialize};

#[test]
fn entity_roundtrip() {
  let mut world1 = World::new();

  world1.spawn().with(Counter::default()).build();
  world1.spawn().with(Duplicator).build();
  world1
    .spawn()
    .with(Counter::default())
    .with(Duplicator)
    .build();
  world1
    .spawn()
    .with(ComponentWithStrangeFriendlyName(-7604))
    .build();

  for _ in 0..10 {
    world1.dispatch_to_all(MsgTick);
    world1.finalize();
  }

  let entities = world1.iter().collect::<Vec<_>>();

  let bin = bincode::serialize(&world1).unwrap();

  let world2: World = bincode::deserialize(&bin).unwrap();

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

    if let Some(w1_strange) =
      world1.query::<&ComponentWithStrangeFriendlyName>(e)
    {
      let w2_strange = world2.query::<&ComponentWithStrangeFriendlyName>(e).unwrap_or_else(|| {
        panic!("{:?} had a component with strange friendly name before serialization but not after", e)
      });
      assert_eq!(
        w1_strange.as_ref(),
        w2_strange.as_ref(),
        "{:?}'s values of strange friendly name mismatched",
        e
      );
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
  world1.insert_resource(ResWithStrangeFriendlyName("hello world".to_string()));

  let bin = bincode::serialize(&world1).unwrap();

  let mut world2: World = bincode::deserialize(&bin).unwrap();

  let a_res1 = world1.get_resource::<AResource>().unwrap();
  let a_res2 = world2.get_resource::<AResource>().unwrap();
  assert_eq!(a_res1, a_res2);

  let odd_name1 = world1.get_resource::<ResWithStrangeFriendlyName>().unwrap();
  let odd_name2 = world2.get_resource::<ResWithStrangeFriendlyName>().unwrap();
  assert_eq!(odd_name1, odd_name2);
}

#[test]
fn roundtrip_all() {
  let mut world1 = World::new();

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

  let bin = bincode::serialize(&world1).unwrap();

  let mut world2: World = bincode::deserialize(&bin).unwrap();

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
  let mut world1 = World::new();
  world1.insert_resource_default::<ResThatIncrementsANumberWhenADuplicatorIsCreated>();

  world1.spawn().with(Duplicator).build();

  for _ in 0..10 {
    world1.dispatch_to_all(MsgTick);
    world1.finalize();
  }

  let bin = bincode::serialize(&world1).unwrap();

  let mut world2: World = bincode::deserialize(&bin).unwrap();

  let rtianwadic1 = world1
    .get_resource::<ResThatIncrementsANumberWhenADuplicatorIsCreated>()
    .unwrap();
  let rtianwadic2 = world2
    .get_resource::<ResThatIncrementsANumberWhenADuplicatorIsCreated>()
    .unwrap();
  // note that we don't actually de/serialize the count in that resource
  assert_eq!(rtianwadic1.count, rtianwadic2.count);
}

// ECM stuff

#[derive(Debug, Serialize, Deserialize, Default, PartialEq, Eq)]
#[register_component]
struct Counter {
  count: u64,
}

impl Component for Counter {
  fn register(builder: ComponentRegisterer<Self>) -> ComponentRegisterer<Self>
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
#[register_component]
struct Duplicator;

impl Component for Duplicator {
  fn register(builder: ComponentRegisterer<Self>) -> ComponentRegisterer<Self>
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

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[register_component]
struct ComponentWithStrangeFriendlyName(i64);

impl Component for ComponentWithStrangeFriendlyName {
  fn register(builder: ComponentRegisterer<Self>) -> ComponentRegisterer<Self>
  where
    Self: Sized,
  {
    builder.set_friendly_name("ligma")
  }
}

#[derive(Resource, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct AResource {
  foo: i32,
  bar: i32,
}

#[derive(Resource, Default, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct DupliCounter {
  count: u64,
}

/// Basically the same as DupliCounter but it does it with a callback
/// and isn't serialized
#[derive(Default, Resource, Serialize, Deserialize)]
struct ResThatIncrementsANumberWhenADuplicatorIsCreated {
  #[serde(skip_deserializing)]
  count: u64,
}

/// Basically the same as DupliCounter but it does it with a callback
/// and isn't serialized
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct ResWithStrangeFriendlyName(String);
manually_register_resource!(ResWithStrangeFriendlyName);

impl Resource for ResWithStrangeFriendlyName {
  fn register(builder: ResourceRegisterer<Self>) -> ResourceRegisterer<Self>
  where
    Self: Sized,
  {
    builder.set_friendly_name("ligma")
  }
}

#[derive(Clone, Message)]
struct MsgTick;
