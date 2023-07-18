use std::{collections::BTreeMap, marker::PhantomData};

use ahash::AHashMap;
use serde::{
  de::{DeserializeOwned, DeserializeSeed, MapAccess, Visitor},
  ser::SerializeMap,
  Serialize, Serializer,
};

use crate::{prelude::World, resource::Resource, TypeIdWrapper};

use super::{SerKey, WorldSerdeInstructions};

// =====================
// === SERIALIZATION ===
// =====================

/// Helper struct for serializing resources.
///
/// Although the internals are exposed, you should probably just be calling [`ResourceSerContext::try_serialize`].
pub struct ResourceSerContext<'a, 'w, Id: SerKey, S: Serializer>
where
  'w: 'a,
{
  /// The map serializer you are to serialize the resources into.
  /// This should have your Id keys mapped to resource values.
  pub map: &'a mut S::SerializeMap,
  /// The resource being serialized.
  pub resource: &'w dyn Resource,
  /// Ids that have already been used. This is used by [`ResourceSerContext::try_serialize`] to check your work
  /// and make sure you don't accidentally use the same key twice.
  extant_ids: AHashMap<Id, TypeIdWrapper>,
}

impl<'a, 'w, Id: SerKey, S: Serializer> ResourceSerContext<'a, 'w, Id, S>
where
  'w: 'a,
{
  fn new(map: &'a mut S::SerializeMap, resource: &'w dyn Resource) -> Self {
    Self {
      map,
      resource,
      extant_ids: AHashMap::new(),
    }
  }

  /// Check if `self.resource` is of the given type, and if it is, serialize it with the given key.
  pub fn try_serialize<R: Resource + Serialize>(
    &mut self,
    id: Id,
  ) -> Result<(), S::Error> {
    let tid = TypeIdWrapper::of::<R>();
    if let Some(extant_tid) = self.extant_ids.insert(id.clone(), tid) {
      if tid != extant_tid {
        panic!(
                    "a serialization key was used for two different resource types, {} and {}",
                    extant_tid.type_name, tid.type_name
                );
      }
    }
    if let Ok(res) = self.resource.downcast_ref::<R>() {
      self.map.serialize_entry(&id, res)?;
    }

    Ok(())
  }
}

/// Wrapper that turns instructions for serializing various resources into something serializable.
///
/// We send this to Serde and pretend it's the whole map.
pub(super) struct ResourcesSerWrapper<
  'w,
  ResId: SerKey,
  CmpId: SerKey,
  W: WorldSerdeInstructions<ResId, CmpId>,
> {
  pub instrs: &'w W,
  pub world: &'w World,

  phantom: PhantomData<*const (ResId, CmpId)>,
}

impl<
    'w,
    ResId: SerKey,
    CmpId: SerKey,
    W: WorldSerdeInstructions<ResId, CmpId>,
  > ResourcesSerWrapper<'w, ResId, CmpId, W>
{
  pub(super) fn new(instrs: &'w W, world: &'w World) -> Self {
    Self {
      instrs,
      world,
      phantom: PhantomData,
    }
  }
}

impl<
    'w,
    ResId: SerKey,
    CmpId: SerKey,
    W: WorldSerdeInstructions<ResId, CmpId>,
  > Serialize for ResourcesSerWrapper<'w, ResId, CmpId, W>
{
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let len = self.instrs.resource_count(self.world);
    let mut map = serializer.serialize_map(len)?;

    for (_, res) in self.world.resources.iter() {
      let lock = res.try_read().unwrap();
      let ctx = ResourceSerContext::<'_, '_, ResId, S>::new(&mut map, &**lock);
      self.instrs.serialize_resource(ctx)?;
    }
    map.end()
  }
}

// =======================
// === DESERIALIZATION ===
// =======================

/// Helper struct for deserializing resources.
pub struct ResourceDeContext<'a, 'de, M: MapAccess<'de>, Id: SerKey> {
  map: M,
  key: Id,
  resources: &'a mut BTreeMap<TypeIdWrapper, Box<dyn Resource>>,

  accepted_resource: bool,
  phantom: PhantomData<&'de ()>,
}

impl<'a, 'de, M: MapAccess<'de>, Id: SerKey> ResourceDeContext<'a, 'de, M, Id> {
  fn new(
    map: M,
    key: Id,
    resources: &'a mut BTreeMap<TypeIdWrapper, Box<dyn Resource>>,
  ) -> Self {
    Self {
      map,
      key,
      resources,
      accepted_resource: false,
      phantom: PhantomData,
    }
  }

  pub fn key(&self) -> Id {
    self.key.clone()
  }

  pub fn accept<R: Resource + DeserializeOwned>(
    &mut self,
  ) -> Result<(), M::Error> {
    if self.accepted_resource {
      panic!("tried to accept a resource twice in a deserialize_resource impl");
    }
    self.accepted_resource = true;

    let tid = TypeIdWrapper::of::<R>();
    let res: R = self.map.next_value()?;
    let prev = self.resources.insert(tid, Box::new(res));
    if prev.is_some() {
      panic!(
        "found the same key twice when deserializing (for type {})",
        tid.type_name
      );
    }

    Ok(())
  }
}

/// Wrapper that reads resources out of a map.
pub(super) struct ResourcesDeWrapper<
  'w,
  ResId: SerKey,
  CmpId: SerKey,
  W: WorldSerdeInstructions<ResId, CmpId>,
> {
  instrs: &'w W,
  phantom: PhantomData<*const (ResId, CmpId)>,
}

impl<
    'w,
    ResId: SerKey,
    CmpId: SerKey,
    W: WorldSerdeInstructions<ResId, CmpId>,
  > ResourcesDeWrapper<'w, ResId, CmpId, W>
{
  pub(super) fn new(instrs: &'w W) -> Self {
    Self {
      instrs,
      phantom: PhantomData,
    }
  }
}

impl<
    'w,
    'de,
    ResId: SerKey,
    CmpId: SerKey,
    W: WorldSerdeInstructions<ResId, CmpId>,
  > DeserializeSeed<'de> for ResourcesDeWrapper<'w, ResId, CmpId, W>
where
  'de: 'w,
{
  type Value = BTreeMap<TypeIdWrapper, Box<dyn Resource>>;

  fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    deserializer.deserialize_map(self)
  }
}

// its its own visitor
impl<
    'w,
    'de,
    ResId: SerKey,
    CmpId: SerKey,
    W: WorldSerdeInstructions<ResId, CmpId>,
  > Visitor<'de> for ResourcesDeWrapper<'w, ResId, CmpId, W>
where
  'de: 'w,
{
  type Value = BTreeMap<TypeIdWrapper, Box<dyn Resource>>;

  fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
    formatter.write_str("a map of resource IDs to resource data")
  }

  fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
  where
    A: MapAccess<'de>,
  {
    let mut out = BTreeMap::new();
    while let Some(key) = map.next_key()? {
      let _: ResId = key;
      let mut ctx = ResourceDeContext::new(map, key, &mut out);
      self.instrs.deserialize_resource(&mut ctx)?;

      if !ctx.accepted_resource {
        panic!("did not accept any resource in a deserialize_resource impl");
      }

      // recover map
      map = ctx.map;
    }
    Ok(out)
  }
}
