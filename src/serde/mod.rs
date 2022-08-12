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
    // The allocator (generational_arena) serializes itself; this is what it happens to look like on the inside.
    // Frankly I'm not really sure what it's doing; the internals of that crate are really smart (it uses a skip list)
    // to compactly store where the free entity slots are, didja know?!)
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

Note that the serialization requires the ability to have keys that aren't strings. So, if you want
to use a human-readable format, json won't work. But [Ron](https://crates.io/crates/ron) works great.
*/

mod entity;
mod resource;
pub use entity::{EntityDeContext, EntitySerContext};
pub use resource::{ResourceDeContext, ResourceSerContext};

use ahash::AHashMap;
use generational_arena::Arena;

use std::{
    collections::{BTreeMap, BTreeSet},
    hash::Hash,
    marker::PhantomData,
};

use serde::{
    de::{self, DeserializeOwned, DeserializeSeed, MapAccess, Visitor},
    Deserialize, Deserializer, Serialize, Serializer,
};

use crate::{
    builder::EntityBuilderComponentTracker,
    entities::{EntityAssoc, EntityStorage},
    prelude::{AccessEntityStats, Entity, World},
    resource::Resource,
    TypeIdWrapper,
};

use self::{
    entity::{EntitiesDeWrapper, EntitySerWrapper},
    resource::{ResourcesDeWrapper, ResourcesSerWrapper},
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
        W: WorldSerdeInstructions<ResId, CmpId>,
        S: Serializer,
        ResId: SerKey,
        CmpId: SerKey,
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
        W: WorldSerdeInstructions<ResId, CmpId>,
        D: Deserializer<'de>,
        ResId: SerKey,
        CmpId: SerKey,
    >(
        &'a mut self,
        instrs: W,
        deserializer: D,
    ) -> Result<(), D::Error>
    where
        'de: 'a,
    {
        let de_world = {
            let seed = DeWorldDeserializer::new(&instrs, &self.known_component_types);
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

        for e in to_callback {
            self.run_creation_callbacks(e);
        }

        Ok(())
    }
}

/// Instructions for serializing and deserializing the various components and resources in the world.
///
/// `ResId` is the key type for resources, and `CmpId` is the key type for components.
pub trait WorldSerdeInstructions<ResId: SerKey, CmpId: SerKey> {
    /// Serialize the components on an entity.
    ///
    /// Although the internals are exposed, for almost all cases you should just be calling
    /// [`EntitySerContext::try_serialize`] for each component type you want to serialize.
    fn serialize_entity<S: Serializer>(
        &self,
        ctx: EntitySerContext<'_, '_, CmpId, S>,
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
        ctx: &mut EntityDeContext<'_, 'de, M, CmpId>,
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

/// Types that can be used as an id when serializing components and resources.
///
/// Although there are a lot of bounds, they should cover anything you care to use as an ID ...
/// and there's a blanket impl to make it even easier.
///
/// I would love to use [`TypeID`](std::any::TypeId) for this and have it happen automatically,
/// but `TypeID`'s specific values aren't stable between rustc versions. So you have to provide it yourself.
pub trait SerKey:
    Clone + Hash + PartialEq + Eq + Serialize + DeserializeOwned + Send + Sync + 'static
{
}

impl<T: Clone + Hash + PartialEq + Eq + Serialize + DeserializeOwned + Send + Sync + 'static> SerKey
    for T
{
}

/// Wrapper for serializing a world.
///
/// This is not actually the same struct as [`DeWorld`]
/// but it has the same signature to serde, so it should Just Work (tm)
#[derive(Serialize)]
#[serde(rename = "SerDeWorld")] // this type does not actually exist, aha
#[serde(bound(serialize = ""))]
struct SerWorld<'a, ResId: SerKey, CmpId: SerKey, W: WorldSerdeInstructions<ResId, CmpId>> {
    allocator: &'a Arena<()>,
    // Maps (real) entities to (fake) instructions for serializing them
    entity_wrappers: AHashMap<Entity, EntitySerWrapper<'a, ResId, CmpId, W>>,
    // Fake resources
    resource_wrappers: ResourcesSerWrapper<'a, ResId, CmpId, W>,
}

/// Wrapper for deserializing a world.
///
/// We can't auto-impl deserialize because we need seeds
struct DeWorld<CmpId: SerKey> {
    allocator: Arena<()>,
    // Maps (real) entities to (fake) instructions for deserializing them
    entity_wrappers: AHashMap<Entity, EntityBuilderComponentTracker>,
    resource_wrappers: BTreeMap<TypeIdWrapper, Box<dyn Resource>>,

    phantom: PhantomData<*const CmpId>,
}

#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "snake_case")]
enum SerdeWorldField {
    Allocator,
    EntityWrappers,
    ResourceWrappers,
}

struct DeWorldDeserializer<
    'a,
    ResId: SerKey,
    CmpId: SerKey,
    W: WorldSerdeInstructions<ResId, CmpId>,
> {
    instrs: &'a W,
    known_component_types: &'a BTreeSet<TypeIdWrapper>,
    phantom: PhantomData<*const (ResId, CmpId)>,
}

impl<'a, ResId: SerKey, CmpId: SerKey, W: WorldSerdeInstructions<ResId, CmpId>>
    DeWorldDeserializer<'a, ResId, CmpId, W>
{
    fn new(instrs: &'a W, known_component_types: &'a BTreeSet<TypeIdWrapper>) -> Self {
        Self {
            instrs,
            known_component_types,
            phantom: PhantomData,
        }
    }
}

impl<'a, 'de, ResId: SerKey, CmpId: SerKey, W: WorldSerdeInstructions<ResId, CmpId>>
    DeserializeSeed<'de> for DeWorldDeserializer<'a, ResId, CmpId, W>
where
    'de: 'a,
{
    type Value = DeWorld<CmpId>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_struct(
            "SerDeWorld",
            &["allocator", "entity_wrappers", "resource_wrappers"],
            self,
        )
    }
}

impl<'a, 'de, ResId: SerKey, CmpId: SerKey, W: WorldSerdeInstructions<ResId, CmpId>> Visitor<'de>
    for DeWorldDeserializer<'a, ResId, CmpId, W>
where
    'de: 'a,
{
    type Value = DeWorld<CmpId>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a serialized world (map or seq)")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: de::SeqAccess<'de>,
    {
        let allocator = seq
            .next_element()?
            .ok_or_else(|| de::Error::invalid_length(0, &self))?;

        let seed = EntitiesDeWrapper::new(self.instrs, self.known_component_types);
        let entity_wrappers = seq
            .next_element_seed(seed)?
            .ok_or_else(|| de::Error::invalid_length(1, &self))?;

        let seed = ResourcesDeWrapper::new(self.instrs);
        let resource_wrappers = seq
            .next_element_seed(seed)?
            .ok_or_else(|| de::Error::invalid_length(2, &self))?;

        Ok(DeWorld {
            allocator,
            entity_wrappers,
            resource_wrappers,

            phantom: PhantomData,
        })
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut allocator = None;
        let mut entity_wrappers = None;
        let mut resource_wrappers = None;
        while let Some(key) = map.next_key()? {
            match key {
                SerdeWorldField::Allocator => {
                    if allocator.is_some() {
                        return Err(de::Error::duplicate_field("allocator"));
                    }
                    allocator = Some(map.next_value()?);
                }
                SerdeWorldField::EntityWrappers => {
                    if entity_wrappers.is_some() {
                        return Err(de::Error::duplicate_field("entity_wrappers"));
                    }

                    let seed = EntitiesDeWrapper::new(self.instrs, self.known_component_types);
                    let wrapper = map.next_value_seed(seed)?;
                    entity_wrappers = Some(wrapper)
                }
                SerdeWorldField::ResourceWrappers => {
                    if resource_wrappers.is_some() {
                        return Err(de::Error::duplicate_field("resource_wrappers"));
                    }

                    let seed = ResourcesDeWrapper::new(self.instrs);
                    let wrapper = map.next_value_seed(seed)?;
                    resource_wrappers = Some(wrapper)
                }
            }
        }

        let allocator = allocator.ok_or_else(|| de::Error::missing_field("allocator"))?;
        let entity_wrappers =
            entity_wrappers.ok_or_else(|| de::Error::missing_field("entity_wrappers"))?;
        let resource_wrappers =
            resource_wrappers.ok_or_else(|| de::Error::missing_field("resource_wrappers"))?;
        Ok(DeWorld {
            allocator,
            entity_wrappers,
            resource_wrappers,

            phantom: PhantomData,
        })
    }
}
