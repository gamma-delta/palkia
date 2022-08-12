mod entity;
pub use entity::{EntityDeContext, EntitySerContext};

use ahash::AHashMap;
use generational_arena::Arena;

use std::{collections::BTreeSet, hash::Hash, marker::PhantomData};

use serde::{
    de::{self, DeserializeOwned, DeserializeSeed, MapAccess, Visitor},
    Deserialize, Deserializer, Serialize, Serializer,
};

use crate::{
    builder::EntityBuilderComponentTracker,
    entities::{EntityAssoc, EntityStorage},
    prelude::{AccessEntityStats, Entity, World},
    TypeIdWrapper,
};

use self::entity::{EntitiesDeWrapper, EntitySerWrapper};

impl World {
    /// Serialize the whole world through the given serializer. This includes all the entities and their
    /// components, their IDs, and resources.
    ///
    /// The `Id` generic is the type components use to identify themselves. See the doc comment for [`SerKey`].
    ///
    /// Note that this uses a serializer, not the front-end `to_string` functions many serde crates provide as convenience.
    /// The workflow will probably look something like
    ///
    /// ```no_run
    /// let mut writer = Vec::new();
    /// let mut serializer = MySerdeSerializer::new(&mut writer);
    /// // The `Ok` variant is often just ()
    /// world.serialize(&mut serializer).unwrap();
    /// String::from_utf8(writer).unwrap();
    /// ```
    ///
    /// See the `serde` tests for practical examples.
    pub fn serialize<W: WorldSerdeInstructions<Id>, S: Serializer, Id: SerKey>(
        &mut self,
        instrs: W,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let entity_wrappers = self
            .iter()
            .map(|e| (e, EntitySerWrapper::new(self, &instrs, e)))
            .collect();

        let allocator = self.entities.allocator.try_read().unwrap();

        // TODO: resources

        let ser_world = SerWorld {
            allocator: &allocator,
            entity_wrappers,
        };
        ser_world.serialize(serializer)
    }

    /// Clears the world, and loads all the entities and resources out of the given deserializer
    /// and into the world.
    ///
    /// You should register your component types, then call this.
    ///
    /// I'm not sure why I need the `instrs` field here to be `&W` and not `W`. It just won't compile
    /// otherwise.
    pub fn deserialize<'a, 'de, W: WorldSerdeInstructions<Id>, D: Deserializer<'de>, Id: SerKey>(
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

        // TODO: resources

        let allocator = de_world.allocator;
        let mut assocs = AHashMap::new();

        for (entity, builder) in de_world.entity_wrappers {
            assert!(allocator.contains(entity.0), "when deserializing, found an entity {:?} marked in the components but not in the allocator", entity);

            let assoc = EntityAssoc::new(builder.components);
            assocs.insert(entity, assoc);
        }

        self.entities = EntityStorage::new(allocator, assocs);
        Ok(())
    }
}

/// Instructions for serializing and deserializing the various components and resources in the world.
pub trait WorldSerdeInstructions<Id: SerKey> {
    /// Serialize the components on an entity.
    ///
    /// Although the internals are exposed, for almost all cases you should just be calling
    /// [`EntitySerContext::try_serialize`] for each component type you want to serialize.
    fn serialize_entity<S: Serializer>(
        &self,
        ctx: EntitySerContext<'_, '_, Id, S>,
    ) -> Result<(), S::Error>;

    /// Get the number of components on the given entity that will be serialized.
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
    /// The code for this should look something like
    ///
    /// ```no_run
    /// match ctx.key {
    ///     
    /// }
    /// ```
    fn deserialize_entity<'a, 'de, M: MapAccess<'de>>(
        &'a self,
        ctx: &mut EntityDeContext<'_, 'de, M, Id>,
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
struct SerWorld<'a, Id: SerKey, W: WorldSerdeInstructions<Id>> {
    allocator: &'a Arena<()>,
    // Maps (real) entities to (fake) instructions for serializing them
    entity_wrappers: AHashMap<Entity, EntitySerWrapper<'a, Id, W>>,
}

/// Wrapper for deserializing a world.
///
/// We can't auto-impl deserialize because we need seeds
struct DeWorld<Id: SerKey> {
    allocator: Arena<()>,
    // Maps (real) entities to (fake) instructions for deserializing them
    entity_wrappers: AHashMap<Entity, EntityBuilderComponentTracker>,
    phantom: PhantomData<*const Id>,
}

#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "snake_case")]
enum SerdeWorldField {
    Allocator,
    EntityWrappers,
}

struct DeWorldDeserializer<'a, Id: SerKey, W: WorldSerdeInstructions<Id>> {
    instrs: &'a W,
    known_component_types: &'a BTreeSet<TypeIdWrapper>,
    phantom: PhantomData<*const Id>,
}

impl<'a, Id: SerKey, W: WorldSerdeInstructions<Id>> DeWorldDeserializer<'a, Id, W> {
    fn new(instrs: &'a W, known_component_types: &'a BTreeSet<TypeIdWrapper>) -> Self {
        Self {
            instrs,
            known_component_types,
            phantom: PhantomData,
        }
    }
}

impl<'a, 'de, Id: SerKey, W: WorldSerdeInstructions<Id>> DeserializeSeed<'de>
    for DeWorldDeserializer<'a, Id, W>
where
    'de: 'a,
{
    type Value = DeWorld<Id>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_struct("SerDeWorld", &["allocator", "entity_wrappers"], self)
    }
}

impl<'a, 'de, Id: SerKey, W: WorldSerdeInstructions<Id>> Visitor<'de>
    for DeWorldDeserializer<'a, Id, W>
where
    'de: 'a,
{
    type Value = DeWorld<Id>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a serialized world")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut allocator = None;
        let mut entity_wrappers = None;
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
            }
        }

        let allocator = allocator.ok_or_else(|| de::Error::missing_field("allocator"))?;
        let entity_wrappers =
            entity_wrappers.ok_or_else(|| de::Error::missing_field("entity_wrappers"))?;
        Ok(DeWorld {
            allocator,
            entity_wrappers,
            phantom: PhantomData,
        })
    }
}
