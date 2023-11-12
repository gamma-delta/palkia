use ahash::AHashMap;
use generational_arena::Arena;

use std::{collections::BTreeMap, marker::PhantomData};

use serde::{
  de::{self, DeserializeSeed, MapAccess, Visitor},
  Deserialize, Deserializer, Serialize,
};

use crate::{
  builder::EntityBuilderComponentTracker, prelude::Entity, resource::Resource,
  TypeIdWrapper,
};

use super::{
  entity::{EntitiesDeWrapper, EntitySerWrapper},
  resource::{ResourcesDeWrapper, ResourcesSerWrapper},
  SerKey, WorldSerdeInstructions,
};

/// Wrapper for serializing a world.
///
/// This is not actually the same struct as [`DeWorld`]
/// but it has the same signature to serde, so it should Just Work (tm)
#[derive(Serialize)]
#[serde(rename = "SerDeWorld")] // this type does not actually exist, aha
#[serde(bound(serialize = ""))]
pub(super) struct SerWorld<'a, ResId: SerKey, W: WorldSerdeInstructions<ResId>>
{
  pub allocator: &'a Arena<()>,
  // Maps (real) entities to (fake) instructions for serializing them
  pub entity_wrappers: AHashMap<Entity, EntitySerWrapper<'a, ResId, W>>,
  // Fake resources
  pub resource_wrappers: ResourcesSerWrapper<'a, ResId, W>,
}

/// Wrapper for deserializing a world.
///
/// We can't auto-impl deserialize because we need seeds
pub(super) struct DeWorld {
  pub allocator: Arena<()>,
  // Maps (real) entities to (fake) instructions for deserializing them
  pub entity_wrappers: AHashMap<Entity, EntityBuilderComponentTracker>,
  pub resource_wrappers: BTreeMap<TypeIdWrapper, Box<dyn Resource>>,
}

#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "snake_case")]
enum SerdeWorldField {
  Allocator,
  EntityWrappers,
  ResourceWrappers,
}

pub(super) struct DeWorldDeserializer<
  'a,
  ResId: SerKey,
  W: WorldSerdeInstructions<ResId>,
> {
  pub instrs: &'a W,
  pub phantom: PhantomData<*const ResId>,
}

impl<'a, ResId: SerKey, W: WorldSerdeInstructions<ResId>>
  DeWorldDeserializer<'a, ResId, W>
{
  pub fn new(instrs: &'a W) -> Self {
    Self {
      instrs,
      phantom: PhantomData,
    }
  }
}

impl<'a, 'de, ResId: SerKey, W: WorldSerdeInstructions<ResId>>
  DeserializeSeed<'de> for DeWorldDeserializer<'a, ResId, W>
where
  'de: 'a,
{
  type Value = DeWorld;

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

impl<'a, 'de, ResId: SerKey, W: WorldSerdeInstructions<ResId>> Visitor<'de>
  for DeWorldDeserializer<'a, ResId, W>
where
  'de: 'a,
{
  type Value = DeWorld;

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

    let seed = EntitiesDeWrapper::new(self.instrs);
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

          let seed = EntitiesDeWrapper::new(self.instrs);
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

    let allocator =
      allocator.ok_or_else(|| de::Error::missing_field("allocator"))?;
    let entity_wrappers = entity_wrappers
      .ok_or_else(|| de::Error::missing_field("entity_wrappers"))?;
    let resource_wrappers = resource_wrappers
      .ok_or_else(|| de::Error::missing_field("resource_wrappers"))?;
    Ok(DeWorld {
      allocator,
      entity_wrappers,
      resource_wrappers,
    })
  }
}
