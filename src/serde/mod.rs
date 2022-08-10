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

use std::{collections::BTreeMap, fmt::Debug, hash::Hash, marker::PhantomData, any::{Any as StdAny, TypeId}};

use ahash::AHashMap;
use downcast::{downcast, Any};
use serde::{de::DeserializeOwned, ser::SerializeMap, Serialize, Serializer};

use crate::{
    entities::{ComponentEntry, EntityAssoc},
    messages::{ListenerWorldAccess, Message},
    prelude::{AccessDispatcher, AccessEntityStats, Component, Entity, World},
    ToTypeIdWrapper, TypeIdWrapper,
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
pub struct MsgSerialize<Id: SerKey, S: Serializer> {
    components: BTreeMap<TypeId, (Id, )>
}
impl<Id: SerKey, S: Serializer> Message for MsgSerialize<Id, S> {}

impl<'s, Id: SerKey, S: Serializer> MsgSerialize<Id, S> {
    fn new() -> Self {
        Self {
            components: BTreeMap::new(),
            phantom: PhantomData
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
pub trait SerDeComponent<Id: SerKey, S: Serializer>:
    Component + Serialize + DeserializeOwned + Any
{
    /// Get the type ID used for this particular component type.
    /// This must be unique across all component types you wish to serialize.
    ///
    /// Please do ***NOT*** use a TypeID for this. See the doc comment on the module for why.
    fn get_id() -> Id;

    /// Default impl of a read handler for [`MsgSerialize`].
    fn serde_handler(
        &self,
        mut msg: MsgSerialize<Id, S>,
        _: Entity,
        _: &ListenerWorldAccess,
    ) -> MsgSerialize<Id, S> {
        let tid = TypeIdWrapper::of::<Self>();
        let clo = |sermap: &mut S::SerializeMap, cmp: &dyn SerDeComponent| {
            // SAFETY: type ID guards
            let id = Self::get_id();
            let cmp: &Self = unsafe { cmp.downcast_ref().unwrap_unchecked() };
            sermap.serialize_entry(&id, cmp)
        };
        msg.components.insert(tid, Box::new(clo));
        msg
    }
}
downcast!(<Id, S> dyn SerDeComponent<Id, S>);

impl World {
    /// Serialize the entities (AND THE ENTITIES ALONE) in the world.
    ///
    /// Empty entities will not be serialized. Put a marker component on them if you want them to stay.
    pub fn serialize_entities<S: Serializer, Id: SerKey>(
        &mut self,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let mut akashic_map = serializer.serialize_map(None)?
        for e in self.iter() {
            let msg = self.dispatch(e, MsgSerialize::<Id, S>::new());

            let sermap = serializer.serialize_map(None)?;
            for (tid, comp) in self.entities.get(e).unwrap().iter() {
                if let Some(serfunc) = msg.components.get(&tid) {
                    let lock = comp.try_read().unwrap();
                    serfunc(&mut sermap, lock.as_any())?;
                }
            }


            entity_map.insert(e, )
        }

        entity_map
            .serialize(serializer)
            .map_err(|e| SerializationError::SerError(e))
    }
}
