//! Blueprint-based entity instantiation loaded from KDL, inspired by Caves of Qud.
//! This used to be a separate crate, `dialga`.
//!
//! Create an empty `EntityFabricator`, add a bunch of KDL files into it,
//! register factories to teach it how to read different KDL lines into components,
//! and then build.

pub mod blueprint;
pub mod factory;

use std::collections::BTreeMap;

use blueprint::{BlueprintLibrary, BlueprintLookupError, BlueprintParseError};
use factory::ComponentFactory;

use serde::de::DeserializeOwned;
use smol_str::SmolStr;
use thiserror::Error;

use crate::{
  builder::EntityBuilder,
  prelude::{Component, Entity},
};

use self::factory::SerdeComponentFactory;

/// A library of blueprints and the ability to instantiate entities from them.
///
/// The `Ctx` generic probably shouldn't need to be `'static`, but I can't figure out how to
/// do it otherwise.
pub struct EntityFabricator<Ctx> {
  blueprints: BlueprintLibrary,
  /// Map component names to factories for it.
  factories: BTreeMap<SmolStr, Box<dyn ComponentFactory<Ctx>>>,
}

impl<Ctx> EntityFabricator<Ctx>
where
  Ctx: 'static,
{
  pub fn new() -> Self {
    Self {
      blueprints: BlueprintLibrary::new(),
      factories: BTreeMap::new(),
    }
  }

  /// Register a component factory.
  pub fn register<CA: ComponentFactory<Ctx>>(
    &mut self,
    name: &str,
    factory: CA,
  ) {
    if let Some(_) = self
      .factories
      .insert(SmolStr::from(name), Box::new(factory))
    {
      panic!("already registered a factory under the name {:?}", name);
    }
  }

  /// Convenience function to register a factory that just loads the thing with serde.
  pub fn register_serde<C: DeserializeOwned + Component>(
    &mut self,
    name: &str,
  ) {
    self.register(name, SerdeComponentFactory::<C, Ctx>::new())
  }

  /// Load the KDL string into the fabricator as a list of blueprints.
  ///
  /// The `filepath` argument is just for error reporting purposes; this doesn't load anything from disc.
  pub fn load_str(
    &mut self,
    src: &str,
    filepath: &str,
  ) -> Result<(), BlueprintParseError> {
    self.blueprints.load_str(src, filepath)
  }

  /// Instantiate an entity from a blueprint, adding all the components in that blueprint
  /// to the builder.
  ///
  /// Note that the builder doesn't have to be empty! For example, you might want to add a component for
  /// its position before filling it with other information.
  pub fn instantiate_to_builder<'a, 'w>(
    &self,
    name: &str,
    mut builder: EntityBuilder<'a, 'w>,
    ctx: &Ctx,
  ) -> Result<EntityBuilder<'a, 'w>, InstantiationError> {
    let print = self.blueprints.lookup(name)?;

    for node in print.components {
      let name = node.name().value();
      let factory = self
        .factories
        .get(name)
        .ok_or_else(|| InstantiationError::NoAssembler(name.into()))?;
      builder = factory
        .assemble(builder, &node, ctx)
        .map_err(|err| InstantiationError::AssemblerError(name.into(), err))?
    }

    Ok(builder)
  }

  /// Convenience method to just return the entity off the builder instead of returning it.
  pub fn instantiate<'a, 'w>(
    &self,
    name: &str,
    builder: EntityBuilder<'a, 'w>,
    ctx: &Ctx,
  ) -> Result<Entity, InstantiationError> {
    Ok(self.instantiate_to_builder(name, builder, ctx)?.build())
  }
}

/// Things that can go wrong when instantiating an entity.
#[derive(Debug, Error)]
pub enum InstantiationError {
  #[error("while looking up the blueprint: {0}")]
  BlueprintLookupError(#[from] BlueprintLookupError),
  #[error("there was no assembler registered for a component named {0:?}")]
  NoAssembler(SmolStr),
  #[error("the assembler for {0:?} gave an error: {1}")]
  AssemblerError(SmolStr, eyre::Error),
}
