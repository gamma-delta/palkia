/*! Serializing and deserializing entities.

Components are stored as simple key-value pairs on each entity, where you provide the key type yourself. [^1]

[^1]: I would love to use [`TypeID`](std::any::TypeId) for this and have it happen automatically,
but `TypeID`'s specific values aren't stable between rustc versions. So you have to provide it yourself.

So, if you choose to use `&'static str` keys, storage for a sample entity might look something like this:

```no-run
an-entity: {
    "position": [1.0, 2.0, 3.0],
    "velocity": [4.0, 5.0, 6.0]
}
```

If you decide instead to use some enum that serializes to an integer as a key, perhaps for compactness,
it will be something like this:

```no-run
an-entity: {
    0: [1.0, 2.0, 3.0],
    1: [4.0, 5.0, 6.0]
}
```

*/

use std::{collections::BTreeMap, fmt::Debug, hash::Hash};

use ahash::AHashMap;
use serde::{de::DeserializeOwned, Serialize, Serializer};
use serde_value::Value as SerdeValue;

use crate::{
    messages::{ListenerWorldAccess, Message},
    prelude::{AccessDispatcher, AccessEntityStats, Component, Entity, World},
    TypeIdWrapper,
};

/// Types that can be used as an id when serializing. This only exists to make the trait bounds less unwieldy.
///
/// Although there are a lot of bounds, they sould cover anything you care to use as an ID ...
/// and there's a blanket impl to make it even easier.
pub trait SerKey: Clone + Hash + PartialEq + Eq + Serialize + Send + Sync + 'static {}

impl<T: Clone + Hash + PartialEq + Eq + Serialize + Send + Sync + 'static> SerKey for T {}

// TODO: this is all shit. Most functional I got was with serde_value, but it didn't round-trip things
// correctly in langs like RON that store structs differently than maps.
//
// So, theory is to clone the serde_value crate and actually support the whole data model.

/// Internal-only-ish message type used to serialize things.
pub struct MsgSerialize<Id: SerKey> {
    components: BTreeMap<TypeIdWrapper, (Id, SerdeValue)>,
    error: Option<serde_value::SerializerError>,
}
impl<Id: SerKey> Message for MsgSerialize<Id> {}

impl<Id: SerKey> MsgSerialize<Id> {
    fn new() -> Self {
        Self {
            components: BTreeMap::new(),
            error: None,
        }
    }
}

/// Trait for components that can be serialized.
///
/// You don't *need* to implement this trait; you can listen to [`MsgSerialize`] yourself
/// if you like. But this is convenient.
///
/// The `Id` generic is the type that components are to be keyed by, and the `S` generic the serializer
/// that can be used. All components in a world that can be serialized must have the same type for this id
/// and the same serializer type. This means you can only ser/de a given world with one kind of serializer.
///
/// ... ok, *technically*, this is not true; if you wanted, you could have two separate type IDs, but then
/// serializing would only work on components with one type of ID at a time ... why do you want to do this?
pub trait SerDeComponent<Id: SerKey>: Component + Serialize + DeserializeOwned {
    /// Get the type ID used for this particular component type.
    /// This must be unique across all component types you wish to serialize.
    ///
    /// Please do ***NOT*** use a TypeID for this. See the doc comment on the module for why.
    fn get_id() -> Id;

    /// Default impl of a read handler for [`MsgSerialize`].
    fn serde_handler(
        &self,
        mut msg: MsgSerialize<Id>,
        _: Entity,
        _: &ListenerWorldAccess,
    ) -> MsgSerialize<Id> {
        if msg.error.is_some() {
            return msg;
        }

        let tid = TypeIdWrapper::of::<Self>();
        let id = Self::get_id();

        let ser = serde_value::to_value(self);
        match ser {
            Ok(it) => {
                msg.components.insert(tid, (id, it));
            }
            Err(err) => msg.error = Some(err),
        }

        msg
    }
}

impl World {
    /// Serialize the entities (AND THE ENTITIES ALONE) in the world.
    ///
    /// Entities with no serializable components will not be serialized.
    /// Put a marker component on them if you want them to stay.
    pub fn serialize_entities<S: Serializer, Id: SerKey>(
        &mut self,
        serializer: S,
    ) -> Result<S::Ok, SerError<S>> {
        let mut entity_map = AHashMap::new();
        for e in self.iter() {
            let msg = self.dispatch(e, MsgSerialize::<Id>::new());

            if let Some(ono) = msg.error {
                return Err(SerError::SerdeValue(ono));
            }

            let mut extant_ids = AHashMap::new();
            let mut sermap = AHashMap::new();
            for (tid, (id, val)) in msg.components {
                if let Some(extant_id) = extant_ids.get(&tid) {
                    if &id != extant_id {
                        panic!("somehow, a component of type {} returned inconsistent values for its ser key. why did you code that. what are you doing.", tid.type_name);
                    }
                } else {
                    extant_ids.insert(tid, id.clone());
                }
                sermap.insert(id, val);
            }

            entity_map.insert(e, sermap);
        }

        entity_map
            .serialize(serializer)
            .map_err(|e| SerError::Ser(e))
    }
}

pub enum SerError<S: Serializer> {
    SerdeValue(serde_value::SerializerError),
    Ser(S::Error),
}

impl<S: Serializer> Debug for SerError<S>
where
    S::Error: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SerdeValue(arg0) => f.debug_tuple("SerdeValue").field(arg0).finish(),
            Self::Ser(arg0) => f.debug_tuple("Ser").field(arg0).finish(),
        }
    }
}
