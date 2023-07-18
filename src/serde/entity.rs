/*! Serializing and deserializing entities.

## Representation

Components are stored as simple key-value pairs on each entity, where you provide the key values yourself.
(It must be something that impls [`SerKey`](super::SerKey).)

So, if you choose to use `&'static str` keys, storage for a sample entity might look something like this:

```json
an-entity: {
    "position": [1.0, 2.0, 3.0],
    "velocity": [4.0, 5.0, 6.0]
}
```

For compactness, you might instead to make up a ComponentType enum with a variant for each component
type you want serialized. You can use the [`serde_repr`](https://crates.io/crates/serde_repr) crate
to have them serialized to integers:

```text
#[derive(Serialize_repr, Deserialize_repr, PartialEq, Debug)]
#[repr(u8)]
enum ComponentType {
    Position, Velocity
}
```

Then, an entity might be serialized like this:

```json
an-entity: {
    0: [1.0, 2.0, 3.0],
    1: [4.0, 5.0, 6.0]
}
```

Entities with no serializable components will not be serialized. Put a marker component on them if you want them to stay.

---

The design is more-or-less stolen from [Hecs' row serialization](https://docs.rs/hecs/0.9.0/hecs/serialize/row/trait.SerializeContext.html).
*/

use std::{collections::BTreeSet, marker::PhantomData};

use ahash::AHashMap;
use serde::{
  de::{DeserializeOwned, DeserializeSeed, MapAccess, Visitor},
  ser::SerializeMap,
  Deserializer, Serialize, Serializer,
};

use crate::{
  builder::EntityBuilderComponentTracker,
  prelude::{AccessQuery, Component, Entity, World},
  TypeIdWrapper,
};

use super::{SerKey, WorldSerdeInstructions};

// =====================
// === SERIALIZATION ===
// =====================

/// Helper struct for serializing entities.
///
/// Although the internals are exposed, you should probably just be calling [`EntitySerContext::try_serialize`].
pub struct EntitySerContext<'a, 'w, Id: SerKey, S: Serializer> {
  /// The map serializer you are to serialize the entities into.
  /// This should have your Id keys mapped to component values.
  pub map: &'a mut S::SerializeMap,
  /// The entity being serialized.
  pub entity: Entity,
  /// Reference to the world, for fetching components.
  pub world: &'w World,
  /// Ids that have already been used. This is used by [`EntitySerContext::try_serialize`] to check your work
  /// and make sure you don't accidentally use the same key twice.
  extant_ids: AHashMap<Id, TypeIdWrapper>,
}

impl<'a, 'w, Id: SerKey, S: Serializer> EntitySerContext<'a, 'w, Id, S> {
  fn new(
    world: &'w World,
    map: &'a mut S::SerializeMap,
    entity: Entity,
  ) -> Self {
    Self {
      map,
      extant_ids: AHashMap::new(),
      world,
      entity,
    }
  }

  /// Convenience function that tries to get a component of the given type off of the world, and if it exists, serializes it.
  /// This automatically checks that you don't accidentally use the same serde key for two types.
  ///
  /// You should call this with every type you wish to be serialized.
  pub fn try_serialize<C: Component + Serialize>(
    &mut self,
    id: Id,
  ) -> Result<(), S::Error> {
    let tid = TypeIdWrapper::of::<C>();
    if let Some(extant_tid) = self.extant_ids.insert(id.clone(), tid) {
      if tid != extant_tid {
        panic!(
                    "a serialization key was used for two different component types, {} and {}",
                    extant_tid.type_name, tid.type_name
                );
      }
    }
    if let Some(comp) = self.world.query::<&C>(self.entity) {
      self.map.serialize_entry(&id, comp.as_ref())?;
    }

    Ok(())
  }
}

/// Wrapper that turns instructions for serializing various components into something serializable.
///
/// Each entity is mapped to one of these. What serde sees is a map of entities to these; this impl
/// then pulls the rug out from under serde and uses the serialization instructions to insert ID->component
/// pairs.
///
/// So, we pretend to Serde that this and [`EntityDeWrapper`] are the same thing.
pub(super) struct EntitySerWrapper<
  'w,
  ResId: SerKey,
  CmpId: SerKey,
  W: WorldSerdeInstructions<ResId, CmpId>,
> {
  pub world: &'w World,
  pub instrs: &'w W,
  pub entity: Entity,

  pub phantom: PhantomData<*const (ResId, CmpId)>,
}

impl<
    'w,
    ResId: SerKey,
    CmpId: SerKey,
    W: WorldSerdeInstructions<ResId, CmpId>,
  > EntitySerWrapper<'w, ResId, CmpId, W>
{
  pub(super) fn new(world: &'w World, instrs: &'w W, entity: Entity) -> Self {
    Self {
      world,
      instrs,
      entity,
      phantom: PhantomData,
    }
  }
}

impl<
    'w,
    ResId: SerKey,
    CmpId: SerKey,
    W: WorldSerdeInstructions<ResId, CmpId>,
  > Serialize for EntitySerWrapper<'w, ResId, CmpId, W>
{
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    let len = self.instrs.component_count(self.entity, self.world);
    let mut map = serializer.serialize_map(len)?;
    let ctx = EntitySerContext::<'_, 'w, CmpId, S>::new(
      self.world,
      &mut map,
      self.entity,
    );
    self.instrs.serialize_entity(ctx)?;
    map.end()
  }
}

// =======================
// === DESERIALIZATION ===
// =======================

// There's a lot of layers here: we have a wrapper around a HashMap<Entity, EntityBuilderComponentTracker>
// (EntitiesDeWrapper), which forwards to a wrapper that reads the EntityBuilderComponentTrackers (EntityDeWrapper)

/// Helper struct for deserializing entities.
pub struct EntityDeContext<'a, 'de, M: MapAccess<'de>, Id: SerKey> {
  map: M,
  tracker: &'a mut EntityBuilderComponentTracker,
  known_types: &'a BTreeSet<TypeIdWrapper>,
  key: Id,

  accepted_entity: bool,
  phantom: PhantomData<&'de ()>,
}

impl<'a, 'de, M: MapAccess<'de>, Id: SerKey> EntityDeContext<'a, 'de, M, Id> {
  fn new(
    map: M,
    tracker: &'a mut EntityBuilderComponentTracker,
    known_types: &'a BTreeSet<TypeIdWrapper>,
    key: Id,
  ) -> Self {
    Self {
      map,
      tracker,
      known_types,
      key,
      accepted_entity: false,
      phantom: PhantomData,
    }
  }

  pub fn key(&self) -> Id {
    self.key.clone()
  }

  /// Signal that the key returned by [`EntityDeContext::key`] is associated with a component of this type.
  ///
  /// Consumes self so you don't accidentally call it twice.
  pub fn accept<C: Component + DeserializeOwned>(
    &mut self,
  ) -> Result<(), M::Error> {
    if self.accepted_entity {
      panic!("tried to accept a component twice in a deserialize_entity impl");
    }
    self.accepted_entity = true;

    let comp: C = self.map.next_value()?;
    self.tracker.insert(comp, self.known_types);
    Ok(())
  }
}

/// Wrapper that reads entity-components pairs out of a map.
pub(super) struct EntitiesDeWrapper<
  'w,
  ResId: SerKey,
  CmpId: SerKey,
  W: WorldSerdeInstructions<ResId, CmpId>,
> {
  instrs: &'w W,
  known_component_types: &'w BTreeSet<TypeIdWrapper>,
  phantom: PhantomData<*const (ResId, CmpId)>,
}

impl<
    'w,
    ResId: SerKey,
    CmpId: SerKey,
    W: WorldSerdeInstructions<ResId, CmpId>,
  > EntitiesDeWrapper<'w, ResId, CmpId, W>
{
  pub(super) fn new(
    instrs: &'w W,
    known_component_types: &'w BTreeSet<TypeIdWrapper>,
  ) -> Self {
    Self {
      instrs,
      known_component_types,
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
  > DeserializeSeed<'de> for EntitiesDeWrapper<'w, ResId, CmpId, W>
where
  'de: 'w,
{
  type Value = AHashMap<Entity, EntityBuilderComponentTracker>;

  fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
  where
    D: Deserializer<'de>,
  {
    deserializer.deserialize_map(self)
  }
}

/// It's its own visitor
impl<
    'w,
    'de,
    ResId: SerKey,
    CmpId: SerKey,
    W: WorldSerdeInstructions<ResId, CmpId>,
  > Visitor<'de> for EntitiesDeWrapper<'w, ResId, CmpId, W>
where
  'de: 'w,
{
  type Value = AHashMap<Entity, EntityBuilderComponentTracker>;

  fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
    formatter.write_str("a map")
  }

  fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
  where
    A: MapAccess<'de>,
  {
    let mut out = AHashMap::with_capacity(map.size_hint().unwrap_or(0));

    while let Some(entity) = map.next_key()? {
      let _: Entity = entity;
      let seed =
        ComponentDeWrapper::new(self.instrs, self.known_component_types);
      let tracker = map.next_value_seed(seed)?;
      out.insert(entity, tracker);
    }

    Ok(out)
  }
}

/// Wrapper that reads component key-value pairs out of a deserializer.
struct ComponentDeWrapper<
  'w,
  ResId: SerKey,
  CmpId: SerKey,
  W: WorldSerdeInstructions<ResId, CmpId>,
> {
  instrs: &'w W,
  known_component_types: &'w BTreeSet<TypeIdWrapper>,
  phantom: PhantomData<*const (ResId, CmpId)>,
}

impl<
    'w,
    ResId: SerKey,
    CmpId: SerKey,
    W: WorldSerdeInstructions<ResId, CmpId>,
  > ComponentDeWrapper<'w, ResId, CmpId, W>
{
  fn new(
    instrs: &'w W,
    known_component_types: &'w BTreeSet<TypeIdWrapper>,
  ) -> Self {
    Self {
      instrs,
      known_component_types,
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
  > DeserializeSeed<'de> for ComponentDeWrapper<'w, ResId, CmpId, W>
where
  'de: 'w,
{
  type Value = EntityBuilderComponentTracker;

  fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
  where
    D: Deserializer<'de>,
  {
    deserializer.deserialize_map(self)
  }
}

// again it is its own visitor
impl<
    'w,
    'de,
    ResId: SerKey,
    CmpId: SerKey,
    W: WorldSerdeInstructions<ResId, CmpId>,
  > Visitor<'de> for ComponentDeWrapper<'w, ResId, CmpId, W>
where
  'de: 'w,
{
  type Value = EntityBuilderComponentTracker;

  fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
    formatter.write_str("a map of component IDs to component data")
  }

  fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
  where
    A: serde::de::MapAccess<'de>,
  {
    let mut tracker = EntityBuilderComponentTracker::default();
    while let Some(key) = map.next_key()? {
      let _: CmpId = key;
      let mut ctx = EntityDeContext::new(
        map,
        &mut tracker,
        &self.known_component_types,
        key,
      );
      self.instrs.deserialize_entity(&mut ctx)?;

      if !ctx.accepted_entity {
        panic!("did not accept any entity in a deserialize_entity impl");
      }

      // Recover the map
      map = ctx.map;
    }

    Ok(tracker)
  }
}
