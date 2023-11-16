use serde::{
  de::{Visitor},
  ser::SerializeMap,
  Deserialize, Serialize,
};

use crate::{
  prelude::{Component},
  vtablesathome::{ComponentVtables},
  ToTypeIdWrapper,
};

use super::{ApplyDeserFn, ErasedSerWrapper};

/// Wrap components in this to serialize them,
/// then get them back by deserializing them into a ComponentDeWrapper.
pub struct ComponentSerWrapper<'w> {
  component: &'w dyn Component,
}

impl<'w> ComponentSerWrapper<'w> {
  pub fn new(component: &'w dyn Component) -> Self {
    Self { component }
  }
}

impl<'w> Serialize for ComponentSerWrapper<'w> {
  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
  where
    S: serde::Serializer,
  {
    let vtable = ComponentVtables::by_tid((*self.component).type_id_wrapper());
    let mut map = serializer.serialize_map(Some(1))?;
    map.serialize_entry(
      vtable.friendly_name,
      &ErasedSerWrapper::new(self.component),
    )?;
    map.end()
  }
}

// ===================
// === DESERIALIZE ===
// ===================

/// Deserialize one component from `{ friendly-name: { data... }}`
pub(super) struct ComponentDeWrapper {
  pub inner: Box<dyn Component>,
}

impl<'de> Deserialize<'de> for ComponentDeWrapper {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: serde::Deserializer<'de>,
  {
    let component = deserializer.deserialize_map(ComponentDeVisitor)?;
    Ok(ComponentDeWrapper { inner: component })
  }
}

/// hey guys, component de visitor heere
struct ComponentDeVisitor;

impl<'de> Visitor<'de> for ComponentDeVisitor {
  type Value = Box<dyn Component>;

  fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
    formatter.write_str(
      "an 'externally tagged' map: `{friendly_name: { ... component ...} }`",
    )
  }

  fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
  where
    A: serde::de::MapAccess<'de>,
  {
    let friendly_name: String = map.next_key()?.ok_or_else(|| {
      <A::Error as serde::de::Error>::custom(
        "requires exactly one key/value pair",
      )
    })?;
    let vtable = ComponentVtables::by_friendly_name(&friendly_name);
    let component = map.next_value_seed(ApplyDeserFn {
      deser: vtable.deser,
    })?;

    // typetag just ignores if there's more than one k/v here, so that's
    // what i'll do i guess

    Ok(component)
  }
}
