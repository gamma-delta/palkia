/*! Serializing and deserializing entities.

---

The design is more-or-less stolen from [Hecs' row serialization](https://docs.rs/hecs/0.9.0/hecs/serialize/row/trait.SerializeContext.html).
*/

use ahash::AHashMap;
use serde::{
  de::{MapAccess, SeqAccess, Visitor},
  ser::{SerializeMap, SerializeSeq},
  Deserialize, Deserializer, Serialize, Serializer,
};

use crate::{
  builder::EntityBuilderComponentTracker,
  prelude::{Entity, World},
  world::EntityAssoc,
};

use super::component::{ComponentDeWrapper, ComponentSerWrapper};

// =====================
// === SERIALIZATION ===
// =====================

/// We pretend to Serde that this and [`EntitiesDeWrapper`] are the same thing.
pub(crate) struct EntitiesSerWrapper<'w> {
  world: &'w World,
}

impl<'w> EntitiesSerWrapper<'w> {
  pub(crate) fn new(world: &'w World) -> Self {
    Self { world }
  }
}

impl<'w> Serialize for EntitiesSerWrapper<'w> {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let mut map = serializer.serialize_map(Some(self.world.entities.len()))?;
    for entity in self.world.entities() {
      map.serialize_key(&entity)?;
      let wrapper = &EntitySerWrapper::new(self.world, entity);
      map.serialize_value(wrapper)?;
    }
    map.end()
  }
}

struct EntitySerWrapper<'w> {
  pub world: &'w World,
  pub entity: Entity,
}

impl<'w> EntitySerWrapper<'w> {
  pub fn new(world: &'w World, entity: Entity) -> Self {
    Self { world, entity }
  }
}

impl<'w> Serialize for EntitySerWrapper<'w> {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let components = self.world.entities.get(self.entity);
    let mut seq = serializer.serialize_seq(Some(components.len()))?;

    for (_tid, assoc) in components.iter() {
      let inner = assoc.read().unwrap();
      let wrapper = ComponentSerWrapper::new(&**inner);
      seq.serialize_element(&wrapper)?;
    }

    seq.end()
  }
}

// =======================
// === DESERIALIZATION ===
// =======================

/// Wrapper that reads a seq of externally tagged component.
pub(super) struct EntitiesDeWrapper {
  pub entities: AHashMap<Entity, EntityAssoc>,
}

impl<'de> Deserialize<'de> for EntitiesDeWrapper {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let entities = deserializer.deserialize_map(EntitiesDeVisitor)?;
    Ok(Self { entities })
  }
}

struct EntitiesDeVisitor;

impl<'de> Visitor<'de> for EntitiesDeVisitor {
  type Value = AHashMap<Entity, EntityAssoc>;

  fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(
      formatter,
      "a map of entities to sequences of externally-tagged components"
    )
  }

  fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
  where
    A: MapAccess<'de>,
  {
    let mut out = AHashMap::new();

    while let Some(entity) = map.next_key()? {
      // force
      let _: Entity = entity;
      let components: EntityDeWrapper = map.next_value()?;

      // how ergonomic
      out.insert(entity, EntityAssoc::new(components.components.components));
    }

    Ok(out)
  }
}

struct EntityDeWrapper {
  components: EntityBuilderComponentTracker,
}

impl<'de> Deserialize<'de> for EntityDeWrapper {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let components = deserializer.deserialize_seq(EntityDeVisitor)?;
    Ok(EntityDeWrapper { components })
  }
}

struct EntityDeVisitor;

impl<'de> Visitor<'de> for EntityDeVisitor {
  type Value = EntityBuilderComponentTracker;

  fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(formatter, "a sequence of externally tagged components")
  }

  fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
  where
    A: SeqAccess<'de>,
  {
    let mut tracker = EntityBuilderComponentTracker::new();
    while let Some(next) = seq.next_element()? {
      // force type
      let _: ComponentDeWrapper = next;
      tracker.insert_raw(next.inner);
    }

    Ok(tracker)
  }
}
