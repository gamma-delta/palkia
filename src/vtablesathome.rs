//! Internal module for getting around the restrictions on Rust's vtables.

use std::collections::BTreeMap;

use crate::{callback::Callbacks, messages::MsgHandlerInner, TypeIdWrapper};

/// Information stored about each component.
pub(crate) struct ComponentVtable {
  pub tid: TypeIdWrapper,
  /// Used for ser/de, both from kdl and to disc
  pub friendly_name: &'static str,
  /// Maps event types to msg handlers
  pub msg_table: BTreeMap<TypeIdWrapper, MsgHandlerInner>,
  pub callbacks: Option<Callbacks>,
}
