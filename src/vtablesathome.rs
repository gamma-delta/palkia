//! Internal module for getting around the restrictions on Rust's vtables.

use std::{any::Any, collections::BTreeMap, sync::OnceLock};

use crate::{
  callback::Callbacks,
  component::ComponentRegistererErased,
  messages::MsgHandlerInner,
  prelude::Component,
  resource::{Resource, ResourceRegistererErased},
  TypeIdWrapper,
};

pub(crate) type DeserializeFn<T> =
  fn(&mut dyn erased_serde::Deserializer) -> erased_serde::Result<Box<T>>;

/// Information stored about each component.
///
/// Public only for the benefit of macros
#[doc(hidden)]
pub struct ComponentVtable {
  pub tid: TypeIdWrapper,
  /// Used for ser/de, both from kdl and to disc
  pub friendly_name: &'static str,
  /// Maps event types to msg handlers
  pub msg_table: BTreeMap<TypeIdWrapper, MsgHandlerInner>,
  pub callbacks: Option<Callbacks>,

  pub deser: DeserializeFn<dyn Component>,
  // Serialization is obj-safe i think?
}

impl ComponentVtable {
  pub fn callbacks(&self) -> Option<&Callbacks> {
    self.callbacks.as_ref()
  }
}

/// Public only for the benefit of macros
#[doc(hidden)]
pub struct ResourceVtable {
  pub tid: TypeIdWrapper,
  pub friendly_name: &'static str,

  pub deser: DeserializeFn<dyn Resource>,
}

pub(crate) fn default_friendly_type_name<T: Any>() -> &'static str {
  std::any::type_name::<T>()
    .split("::")
    .last()
    .expect("somehow had a type with no name")
}

// todo these have a ton of duplicated code auauau

/// Static registry of components
pub(crate) struct ComponentVtables {
  tables: Vec<ComponentVtable>,
  by_tid: BTreeMap<TypeIdWrapper, usize>,
  by_friendly_name: BTreeMap<String, usize>,
}

static COMPONENT_VTABLES: OnceLock<ComponentVtables> = OnceLock::new();

impl ComponentVtables {
  fn get_inner() -> &'static ComponentVtables {
    COMPONENT_VTABLES.get_or_init(|| {
      let mut me = ComponentVtables {
        tables: Vec::new(),
        by_tid: BTreeMap::default(),
        by_friendly_name: BTreeMap::default(),
      };
      for registrator in crate::__private::COMPONENT_REGISTRATORS {
        let erased = ComponentRegistererErased::new();
        let vtable = registrator(erased);

        let idx = me.tables.len();

        if me.by_tid.contains_key(&vtable.tid) {
          eprintln!(
            "tried to register component type {} twice",
            vtable.tid.type_name
          );
          continue;
        }
        if let Some(ono) = me.by_friendly_name.get(vtable.friendly_name) {
          eprintln!(
            "duplicate friendly component name {}:
            originally registered by {}, now trying by {}",
            &vtable.friendly_name,
            me.tables[*ono].tid.type_name,
            vtable.tid.type_name
          );
          continue;
        }

        me.by_tid.insert(vtable.tid, idx);
        me.by_friendly_name
          .insert(vtable.friendly_name.to_owned(), idx);
        me.tables.push(vtable);
      }
      me
    })
  }

  pub(crate) fn by_type<C>() -> &'static ComponentVtable
  where
    C: Component,
  {
    Self::by_tid(TypeIdWrapper::of::<C>())
  }

  pub(crate) fn by_tid(tid: TypeIdWrapper) -> &'static ComponentVtable {
    let vtables = Self::get_inner();
    let idx = vtables.by_tid.get(&tid).unwrap_or_else(|| {
      panic!(
        "tried to access component of type {} without registering it",
        tid.type_name
      )
    });
    &vtables.tables[*idx]
  }

  pub(crate) fn by_friendly_name(name: &str) -> &'static ComponentVtable {
    let vtables = Self::get_inner();
    let idx = vtables.by_friendly_name.get(name).unwrap_or_else(|| {
      panic!(
        "tried to access component with unknown friendly name {:?}",
        name,
      )
    });
    &vtables.tables[*idx]
  }
}
/// Static registry of resources
pub(crate) struct ResourceVtables {
  tables: Vec<ResourceVtable>,
  by_tid: BTreeMap<TypeIdWrapper, usize>,
  by_friendly_name: BTreeMap<String, usize>,
}

static RESOURCE_VTABLES: OnceLock<ResourceVtables> = OnceLock::new();

impl ResourceVtables {
  fn get_inner() -> &'static ResourceVtables {
    RESOURCE_VTABLES.get_or_init(|| {
      let mut me = ResourceVtables {
        tables: Vec::new(),
        by_tid: BTreeMap::default(),
        by_friendly_name: BTreeMap::default(),
      };
      for registrator_fn in crate::__private::RESOURCE_REGISTRATORS {
        let erased = ResourceRegistererErased::new();
        let vtable = registrator_fn(erased);

        let idx = me.tables.len();

        if me.by_tid.contains_key(&vtable.tid) {
          eprintln!(
            "tried to register resource type {} twice",
            vtable.tid.type_name
          );
          continue;
        }
        if let Some(ono) = me.by_friendly_name.get(vtable.friendly_name) {
          eprintln!(
            "duplicate friendly resource name {:?}:
            originally registered by {}, now trying by {}",
            &vtable.friendly_name,
            me.tables[*ono].tid.type_name,
            vtable.tid.type_name
          );
          continue;
        }

        me.by_tid.insert(vtable.tid, idx);
        me.by_friendly_name
          .insert(vtable.friendly_name.to_owned(), idx);
        me.tables.push(vtable);
      }
      me
    })
  }

  pub(crate) fn by_tid(tid: TypeIdWrapper) -> &'static ResourceVtable {
    let vtables = Self::get_inner();
    let idx = vtables.by_tid.get(&tid).unwrap_or_else(|| {
      panic!(
        "tried to access resource of type {} without registering it",
        tid.type_name
      )
    });
    &vtables.tables[*idx]
  }

  pub(crate) fn by_friendly_name(name: &str) -> &'static ResourceVtable {
    let vtables = Self::get_inner();
    let idx = vtables.by_friendly_name.get(name).unwrap_or_else(|| {
      panic!(
        "tried to access resource with unknown friendly name {:?}",
        name,
      )
    });
    &vtables.tables[*idx]
  }
}
