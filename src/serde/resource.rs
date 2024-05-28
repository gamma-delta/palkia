use serde::{
  de::{MapAccess, Visitor},
  ser::SerializeMap,
  Deserialize, Serialize, Serializer,
};

use crate::{
  prelude::World, vtablesathome::ResourceVtables, world::storage::ResourceMap,
};

use super::{ApplyDeserFn, ErasedSerWrapper};

// =====================
// === SERIALIZATION ===
// =====================

/// Wrapper that turns instructions for serializing various resources into something serializable.
///
/// We send this to Serde and pretend it's the whole map.
pub(super) struct ResourcesSerWrapper<'w> {
  pub world: &'w World,
}

impl<'w> ResourcesSerWrapper<'w> {
  pub(super) fn new(world: &'w World) -> Self {
    Self { world }
  }
}

impl<'w> Serialize for ResourcesSerWrapper<'w> {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let len = self.world.resources.len();

    let mut map = serializer.serialize_map(Some(len))?;

    for (tid, res) in self.world.resources.iter() {
      let vtable = ResourceVtables::by_tid(tid);
      let lock = res.read().unwrap();

      map.serialize_key(vtable.friendly_name)?;
      map.serialize_value(&ErasedSerWrapper::new(&**lock))?;
    }
    map.end()
  }
}

// =======================
// === DESERIALIZATION ===
// =======================

pub(super) struct ResourcesDeWrapper {
  pub resources: ResourceMap,
}

impl<'de> Deserialize<'de> for ResourcesDeWrapper {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    let resources = deserializer.deserialize_map(ResourcesDeVisitor)?;
    Ok(ResourcesDeWrapper { resources })
  }
}

struct ResourcesDeVisitor;

impl<'de> Visitor<'de> for ResourcesDeVisitor {
  type Value = ResourceMap;

  fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(formatter, "a map of friendly names to resources")
  }

  fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
  where
    A: MapAccess<'de>,
  {
    let mut out = ResourceMap::new();
    while let Some(key) = map.next_key()? {
      // force type
      let _: String = key;
      let vtable = ResourceVtables::by_friendly_name(&key);
      let res = map.next_value_seed(ApplyDeserFn {
        deser: vtable.deser,
      })?;
      out.insert_raw(res);
    }

    Ok(out)
  }
}
