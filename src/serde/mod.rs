mod entity;
use ahash::AHashMap;
pub use entity::{MsgSerialize, SerDeComponent};
use generational_arena::Arena;

use std::{fmt::Debug, hash::Hash};

use serde::{Serialize, Serializer};
use serde_value::Value as SerdeValue;

use crate::prelude::{Entity, World};

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
    pub fn serialize<S: Serializer, Id: SerKey>(
        &mut self,
        serializer: S,
    ) -> Result<S::Ok, SerError<S>> {
        let entity_data = entity::serialize_entities::<Id>(self).map_err(SerError::SerdeValue)?;

        let allocator = self.entities.allocator.get_mut().unwrap();
        let ser_world = SerWorld {
            entity_data,
            allocator,
        };
        ser_world.serialize(serializer).map_err(SerError::Ser)
    }
}

/// Wrapper for a world when serializing.
#[derive(Serialize)]
struct SerWorld<'w, Id: SerKey> {
    entity_data: AHashMap<Entity, AHashMap<Id, SerdeValue>>,
    allocator: &'w Arena<()>,
}

/// Types that can be used as an id when serializing components and resources.
///
/// Although there are a lot of bounds, they should cover anything you care to use as an ID ...
/// and there's a blanket impl to make it even easier.
///
/// I would love to use [`TypeID`](std::any::TypeId) for this and have it happen automatically,
/// but `TypeID`'s specific values aren't stable between rustc versions. So you have to provide it yourself.
pub trait SerKey: Clone + Hash + PartialEq + Eq + Serialize + Send + Sync + 'static {}

impl<T: Clone + Hash + PartialEq + Eq + Serialize + Send + Sync + 'static> SerKey for T {}

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
