/*!
Serializing and deserializing worlds.

Worlds are stored as:

- a mapping of user-defined keys to resource data
- the backing allocator for the entities
- a mapping of entities to, a mapping of "friendly-name" keys to component data.

Although some of the internals of this module are exposed, in practice you
should just have to call
[`World::serialize`] and [`World::deserialize`], and it should Just Work(tm).

In pseudo-Ron, a serialized world will look something like this:

```text
SerDeWorld(
    // The allocator (generational_arena) serializes itself;
    // this is what it happens to look like on the inside.
    // Frankly I'm not really sure what it's doing; the internals of that crate are
    // really smart.
    // (It uses a skip list to compactly store where the free entity slots are,
    // didja know?!)
    allocator: [
        Some(0, ()),
        Some(1, ()),
        Some(2, ()),
        Some(3, ()),
        None, None, None, None
    ],
    resources: {
        // Assuming you have some struct MyResource { foo: i32, bar: i32 }
        "my_resource": (foo: 10, bar: 20),
        "my_other_resource": (baz: "fizzbuzz", quxx: (spam: "eggs")),
        ...
    },
    entities: {
        // Entities are stored as [index, generation]
        [0,0]: [
            {"position": [0.0, 1.0, 2.0]},
            {"velocity": [0.1, 0.2, 0.3]},
            {"player": ()},
        ],
        [1,0]: {
            {"position": [0.0, 1.0, 2.0]},
            {"velocity": [0.1, 0.2, 0.3]},
        },
        [2,0]: {
            {"position": [0.0, 1.0, 2.0]},
            {"velocity": [0.1, 0.2, 0.3]},
            {"collider": ()},
        },
        ...
    }
)
```

Note that after deserializing, world insertion callbacks WILL be called
 So, if you're using those callbacks to
create a cache, like for (say) entity positions, then you
shouldn't serialize whatever you're caching, or invalidate it before you
load it.

---

Note that the entity serialization requires the ability to have keys
that aren't strings. So, if you want to use a human-readable format,
json won't work. But [Ron](https://crates.io/crates/ron) works great.

For something compact, remember that a lot of binary formats aren't amazingly
compatible when the schema changes.
I personally haven't looked into this, but it might be worth using something
like [MessagePack](https://github.com/3Hren/msgpack-rust)
which serializes struct field names so you can change component definitions
without breaking things.

If you're worried about this leading to gigantic save files, gzipping it should
probably help a lot.

But, you can freely add *new* component types as you develop a game, and old saves should be compatible.

*/

mod component;
mod entity;
mod resource;

use generational_arena::Arena;

use serde::{
  de::DeserializeSeed, Deserialize, Deserializer, Serialize, Serializer,
};

use crate::{
  prelude::World, vtablesathome::DeserializeFn, world::storage::EntityStorage,
};

use self::{
  entity::{EntitiesDeWrapper, EntitiesSerWrapper},
  resource::{ResourcesDeWrapper, ResourcesSerWrapper},
};

impl Serialize for World {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let allocator = self.entities.allocator.try_read().unwrap();
    let entities = EntitiesSerWrapper::new(self);
    let resources = ResourcesSerWrapper::new(self);

    let wrapper = WorldSerWrapper {
      allocator: &*allocator,
      entities,
      resources,
    };
    wrapper.serialize(serializer)
  }
}

impl<'de> Deserialize<'de> for World {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let wrapper = WorldDeWrapper::deserialize(deserializer)?;

    let mut world = World::new();
    // do i ... repeat, repeat myself?
    world.resources = wrapper.resources.resources;
    world.entities =
      EntityStorage::new(wrapper.allocator, wrapper.entities.entities);

    for e in world.entities() {
      world.run_creation_callbacks(e);
    }
    world.finalize();
    Ok(world)
  }
}

#[derive(Serialize)]
struct WorldSerWrapper<'w> {
  allocator: &'w Arena<()>,
  entities: EntitiesSerWrapper<'w>,
  resources: ResourcesSerWrapper<'w>,
}

#[derive(Deserialize)]
struct WorldDeWrapper {
  allocator: Arena<()>,
  entities: EntitiesDeWrapper,
  resources: ResourcesDeWrapper,
}

struct ErasedSerWrapper<'a, T: ?Sized> {
  inner: &'a T,
}

impl<'a, T: ?Sized> ErasedSerWrapper<'a, T> {
  fn new(inner: &'a T) -> Self {
    Self { inner }
  }
}

impl<'a, T: ?Sized> Serialize for ErasedSerWrapper<'a, T>
where
  T: erased_serde::Serialize,
{
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    erased_serde::serialize(self.inner, serializer)
  }
}

/// Deserializer that applies the deser fn to an erased deserializer.
/// Used for component/resource deserialization.
///
/// Thanks typetag for notes here.
struct ApplyDeserFn<T: ?Sized> {
  deser: DeserializeFn<T>,
}

impl<'de, T> DeserializeSeed<'de> for ApplyDeserFn<T>
where
  T: ?Sized,
{
  type Value = Box<T>;

  fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
  where
    D: Deserializer<'de>,
  {
    // it's automatically maintainable and readable because it's written
    // in crab language
    let mut erased = <dyn erased_serde::Deserializer>::erase(deserializer);
    (self.deser)(&mut erased).map_err(serde::de::Error::custom)
  }
}
