use std::marker::PhantomData;

use kdl::KdlNode;
use serde::de::DeserializeOwned;

use crate::{builder::EntityBuilder, prelude::Component};

/// Do one step of building an entity from a node. Usually, implementors will:
/// - Deser a component out of the node
/// - Add it to the builder
///
/// Each assembler is a singleton object stored in an [`EntityFabricator`].
/// You can use the `&self` param for configuration data, I suppose.
pub trait ComponentFactory<Ctx>: Send + Sync + 'static
where
  Ctx: 'static,
{
  /// Attempt to load a component out of a node with full access to the builder.
  fn assemble<'a, 'w>(
    &self,
    builder: EntityBuilder<'a, 'w>,
    node: &KdlNode,
    ctx: &Ctx,
  ) -> eyre::Result<EntityBuilder<'a, 'w>>;
}

/// Convenience wrapper for the common case where you want to just deserialize something from
/// a node with serde.
///
/// Doesn't use the `Ctx` generic (just has it in PhantomData).
// the funky generic in the Phantom Data is due to irritating send/sync reasons
pub struct SerdeComponentFactory<T, Ctx>(PhantomData<fn(&Ctx) -> T>);

impl<T, Ctx> SerdeComponentFactory<T, Ctx> {
  pub fn new() -> Self {
    Self(PhantomData)
  }
}

impl<T, Ctx> ComponentFactory<Ctx> for SerdeComponentFactory<T, Ctx>
where
  Self: 'static,
  T: DeserializeOwned + Component,
{
  fn assemble<'a, 'w>(
    &self,
    mut builder: EntityBuilder<'a, 'w>,
    node: &KdlNode,
    _ctx: &Ctx,
  ) -> eyre::Result<EntityBuilder<'a, 'w>> {
    let comp: T = knurdy::deserialize_node(node)?;
    builder.insert(comp);
    Ok(builder)
  }
}
