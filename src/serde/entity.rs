/*! Serializing and deserializing entities.

To set up a component type to be stored, implement [`SerDeComponent::get_id`] on it, then use the default
impl of [`SerDeComponent::ser_handler`] as a read listener when registering listeners for it.

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

```no_run
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
*/

use std::collections::BTreeMap;

use ahash::AHashMap;
use bimap::BiHashMap;
use serde::{
    de::{DeserializeOwned, Visitor},
    ser::SerializeMap,
    Deserializer, Serialize, Serializer,
};
use serde_value::Value as SerdeValue;

use crate::{
    entities::EntityStorage,
    messages::{ListenerWorldAccess, Message},
    prelude::{AccessDispatcher, AccessEntityStats, Component, Entity, World},
    resource::Resource,
    TypeIdWrapper,
};

use super::SerKey;

// i love generics i love making code that is very readable
pub(crate) struct ComponentSerdeInstrs<Id, S, D>
where
    Id: SerKey,
    S: Serializer,
    D: for<'a> Deserializer<'a>,
{
    ids: BiHashMap<Id, TypeIdWrapper>,
    instrs: AHashMap<TypeIdWrapper, ComponentSerdeInstr<Id, S, D>>,
}

impl<Id, S, D> ComponentSerdeInstrs<Id, S, D>
where
    Id: SerKey,
    S: Serializer,
    D: for<'a> Deserializer<'a>,
{
    pub fn new() -> Self {
        Self {
            ids: BiHashMap::new(),
            instrs: AHashMap::new(),
        }
    }

    pub fn register<SDC: SerDeComponent<Id, S, D>>(&mut self) {
        let id = SDC::get_id();
        let tid = TypeIdWrapper::of::<SDC>();
        if let Some(extant_id) = self.ids.get_by_right(&tid) {
            if extant_id != &id {
                panic!("somehow, a component of type {} returned inconsistent values for its ser key. why did you code that. what are you doing.", tid.type_name);
            }
        }

        self.instrs.insert(
            tid,
            ComponentSerdeInstr {
                ser: SDC::ser_instr,
                de: SDC::de_instr,
            },
        )
    }

    pub fn serialize(
        &self,
        serializer: &mut S,
        entities: &EntityStorage,
    ) -> Result<S::Ok, S::Error> {
        let mut entity_map = serializer.serialize_map(None)?;
        for e in entities.iter() {
            let assoc = entities.get(e);
            let len = assoc
                .iter()
                .filter(|(tid, _)| self.instrs.contains_key(tid))
                .count();
            // TODO: serialize *wrapper* that forwards to the relevant ser/de entry
            for (tid, data) in assoc.iter() {
                if let Some(key) = self.ids.get_by_right(&tid) {
                    let instrs = &self.instrs[&tid];
                    let lock = data.try_read().unwrap();
                    instrs.ser(key.to_owned(), lock.as_ref(), &mut map)?;
                }
            }
            map.end()?;
        }
    }
}

impl<Id, S, D> Resource for ComponentSerdeInstrs<Id, S, D>
where
    Id: SerKey,
    S: Serializer,
    D: for<'a> Deserializer<'a> + 'static,
{
}

pub(crate) struct ComponentSerdeInstr<Id, S, D>
where
    Id: SerKey,
    S: Serializer,
    D: for<'a> Deserializer<'a>,
{
    ser: fn(Id, &dyn Component, &mut S::SerializeMap) -> Result<(), S::Error>,
    de: fn(D) -> Result<Box<dyn Component>, S::Error>,
}

struct EntitySerializerWrapper<'a, Id, S> where Id: SerKey, S: Serializer {
    ser_instrs: fn(Id, &dyn Component, &mut S::SerializeMap) -> Result<(), S::Error>,
    comp: &'a dyn Component,
}

impl<'a> Serialize for EntitySerializer<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer {
        let mut map = serializer.serialize_map()
    }
}

/// Trait for components that can be serialized.
///
/// This trait is more or less a generator for the [`SerDeComponent::ser_handler`] function, which is a
/// read listener for [`MsgSerialize`].
///
/// The `Id` generic is the type that components are to be keyed by, and the `S` generic the serializer
/// that can be used. All components in a world that can be serialized must have the same type for this id
/// and the same serializer type. This means you can only ser/de a given world with one kind of serializer.
///
/// ... ok, *technically*, this is not true; if you wanted, you could have two separate type IDs, but then
/// serializing would only work on components with one type of ID at a time ... why do you want to do this?
pub trait SerDeComponent<Id: SerKey, S: Serializer, D: for<'a> Deserializer<'a>>:
    Component + Serialize + DeserializeOwned
{
    /// Get the type ID used for this particular component type.
    /// This must be unique across all component types you wish to serialize.
    ///
    /// Please do ***NOT*** use a TypeID for this. See the doc comment on the module for why.
    fn get_id() -> Id;

    fn ser_instr(
        id: Id,
        this: &dyn Component,
        serializer: &mut S::SerializeMap,
    ) -> Result<(), S::Error> {
        // Won't do an unchecked cast here cause this very well might fail due to bad at coding
        // probably can in the future tho
        let really_this: &Self = this.downcast_ref().unwrap();
        serializer.serialize_entry(&id, really_this)
    }

    fn de_instr(deserializer: D) -> Result<Box<dyn Component>, S::Error> {
        Self::deserialize(deserializer).map(Box::new)
    }
}
