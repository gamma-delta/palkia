/*!
Serializing and deserializing worlds.

Worlds are stored as:

- a mapping of user-defined keys to resource data
- the backing allocator for the entities
- a mapping of entities to, a mapping of user-defined keys to component data.

This "user-defined key" is parameterized as [`SerKey`], which is automatically implemented
for any hashable, cloneable, ser/deable type. You should probably use an enum for this type.

Although some of the internals of this module are exposed, in practice you should just have to call
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
        [0,0]: {
            "position": [0.0, 1.0, 2.0],
            "velocity": [0.1, 0.2, 0.3],
            "player": (),
        },
        [1,0]: {
            "position": [0.0, 1.0, 2.0],
            "velocity": [0.1, 0.2, 0.3],
        },
        [2,0]: {
            "position": [0.0, 1.0, 2.0],
            "velocity": [0.1, 0.2, 0.3],
            "collider": (),
        },
        ...
    }
)
```

Note that after deserializing, world insertion callbacks WILL be called! So, if you're using those callbacks to
create a cache, like for (say) entity positions, then you shouldn't serialize whatever you're caching.

---

Note that the serialization requires the ability to have keys that aren't strings. So, if you want to use a human-readable format,
json won't work. But [Ron](https://crates.io/crates/ron) works great.

For something compact, remember that a lot of binary formats aren't amazingly compatible when the schema changes.
I personally haven't looked into this, but it might be worth using something like [MessagePack](https://github.com/3Hren/msgpack-rust)
which serializes struct field names so you can change component definitions without breaking things.

But, you can freely add *new* component types as you develop a game, and old saves should be compatible.

*/

mod entity;
mod resource;
mod wrapper;
pub use entity::{EntityDeContext, EntitySerContext};
pub use resource::{ResourceDeContext, ResourceSerContext};

use ahash::AHashMap;

use std::hash::Hash;

use serde::{
  de::{DeserializeOwned, DeserializeSeed, MapAccess},
  Deserializer, Serialize, Serializer,
};

use crate::{
  prelude::{AccessEntityStats, Entity, World},
  world::storage::{EntityAssoc, EntityStorage},
};

use self::{
  entity::EntitySerWrapper,
  resource::ResourcesSerWrapper,
  wrapper::{DeWorldDeserializer, SerWorld},
};

impl World {
  /// Serialize the whole world through the given serializer. This includes all the entities and their
  /// components, their IDs, and resources.
  ///
  /// The `Id` generic is the type components use to identify themselves. See the doc comment for [`SerKey`].
  ///
  /// Note that this uses a serializer, not the front-end `to_string` functions many serde crates provide as convenience.
  /// The workflow will probably look something like
  ///
  /// ```text
  /// let mut writer = Vec::new();
  /// let mut serializer = MySerdeSerializer::new(&mut writer);
  /// // The `Ok` variant is often just ()
  /// world.serialize(&mut serializer).unwrap();
  /// String::from_utf8(writer).unwrap();
  /// ```
  ///
  /// See the `serde` tests for practical examples.
  pub fn serialize<
    W: WorldSerdeInstructions<ResId>,
    S: Serializer,
    ResId: SerKey,
  >(
    &mut self,
    instrs: W,
    serializer: S,
  ) -> Result<S::Ok, S::Error> {
    let allocator = self.entities.allocator.try_read().unwrap();
    let entity_wrappers = self
      .iter()
      .map(|e| (e, EntitySerWrapper::new(self, &instrs, e)))
      .collect();

    let resource_wrappers = ResourcesSerWrapper::new(&instrs, self);

    let ser_world = SerWorld {
      allocator: &allocator,
      entity_wrappers,
      resource_wrappers,
    };
    ser_world.serialize(serializer)
  }

  /// Clears the entities in the world, and loads all the entities and resources out of the given deserializer
  /// and into the world.
  ///
  /// If a resource is found both in the serialized data and the world, the serialized resource will replace the
  /// present one, but old resources will stick around.
  ///
  /// You should register your component types, then call this. (There will be panics otherwise.)
  pub fn deserialize<
    'a,
    'de,
    W: WorldSerdeInstructions<ResId>,
    D: Deserializer<'de>,
    ResId: SerKey,
  >(
    &'a mut self,
    instrs: W,
    deserializer: D,
  ) -> Result<(), D::Error>
  where
    'de: 'a,
  {
    let de_world = {
      let seed = DeWorldDeserializer::new(&instrs);
      seed.deserialize(deserializer)?
    };

    for (_, res) in de_world.resource_wrappers {
      self.resources.insert_raw(res);
    }

    let allocator = de_world.allocator;
    let mut assocs = AHashMap::new();

    let mut to_callback = Vec::with_capacity(de_world.entity_wrappers.len());
    for (entity, builder) in de_world.entity_wrappers {
      assert!(allocator.contains(entity.0), "when deserializing, found an entity {:?} marked in the components but not in the allocator", entity);

      let assoc = EntityAssoc::new(builder.components);
      assocs.insert(entity, assoc);

      to_callback.push(entity);
    }
    self.entities = EntityStorage::new(allocator, assocs);

    // all the entities are created at once, so call callbacks after
    for e in to_callback {
      self.run_creation_callbacks(e);
    }

    Ok(())
  }
}

/// Instructions for serializing and deserializing the various components and resources in the world.
///
/// `ResId` is the key type for resources, and `CmpId` is the key type for components.
pub trait WorldSerdeInstructions<ResId: SerKey> {
  /// Serialize the components on an entity.
  ///
  /// Although the internals are exposed, for almost all cases you should just be calling
  /// [`EntitySerContext::try_serialize`] for each component type you want to serialize.
  fn serialize_entity<S: Serializer>(
    &self,
    ctx: EntitySerContext<'_, '_, S>,
  ) -> Result<(), S::Error>;

  /// Return the number of serializable components on the given entity.
  ///
  /// Certain serializers require the number of items in a map to be known before the map is serialized,
  /// so if you're using one of those you must implement this method. By default, it returns `None`.
  fn component_count(&self, entity: Entity, world: &World) -> Option<usize> {
    let _ = entity;
    let _ = world;
    None
  }

  /// Try to deserialize the given component from an entity.
  ///
  /// See the serde tests for how the implementation should look.
  fn deserialize_entity<'a, 'de, M: MapAccess<'de>>(
    &'a self,
    ctx: &mut EntityDeContext<'_, 'de, M>,
  ) -> Result<(), M::Error>
  where
    'de: 'a;

  /// Serialize a resource.
  ///
  /// For almost all cases you should just be calling [`ResourceSerContext::try_serialize`] for each
  /// resource type you'd like to serialize.
  fn serialize_resource<S: Serializer>(
    &self,
    ctx: ResourceSerContext<'_, '_, ResId, S>,
  ) -> Result<(), S::Error>;

  /// Return the number of serializable resources on the world.
  ///
  /// Certain serializers require the number of items in a map to be known before the map is serialized,
  /// so if you're using one of those you must implement this method. By default, it returns `None`.
  fn resource_count(&self, world: &World) -> Option<usize> {
    let _ = world;
    None
  }

  fn deserialize_resource<'a, 'de, M: MapAccess<'de>>(
    &'a self,
    ctx: &mut ResourceDeContext<'_, 'de, M, ResId>,
  ) -> Result<(), M::Error>
  where
    'de: 'a;
}

/// Types that can be used as an id when serializing resources.
///
/// Although there are a lot of bounds, they should cover anything you care to use as an ID ...
/// and there's a blanket impl to make it even easier.
///
/// I would love to use [`TypeID`](std::any::TypeId) for this and have it happen automatically,
/// but `TypeID`'s specific values aren't stable between rustc versions. So you have to provide it yourself.
///
/// TODO: as of recently, components are round-tripped with their friendly
/// names. Resources should do something similar
pub trait SerKey:
  Clone
  + Hash
  + PartialEq
  + Eq
  + Serialize
  + DeserializeOwned
  + Send
  + Sync
  + 'static
{
}

impl<
    T: Clone
      + Hash
      + PartialEq
      + Eq
      + Serialize
      + DeserializeOwned
      + Send
      + Sync
      + 'static,
  > SerKey for T
{
}
