use std::marker::PhantomData;

use ahash::AHashSet;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::prelude::{Component, Entity};

/// Keep track of the entities that have a specific component on them.
///
/// You can't add this directly to a world because Palkia doesn't support generic Resources.
/// Put it as a newtype struct.
#[derive(Serialize, Deserialize)]
pub struct TrackEntitiesWithComponent<C: 'static> {
  /// this gets reconstructed at ser/de time
  #[serde(skip)]
  pub(crate) entities: AHashSet<Entity>,
  _phantom: PhantomData<C>,
}

impl<C> TrackEntitiesWithComponent<C>
where
  C: Component + 'static,
{
  pub fn new() -> Self {
    Self {
      entities: AHashSet::new(),
      _phantom: PhantomData,
    }
  }

  pub fn has_entity(&self, e: Entity) -> bool {
    self.entities.contains(&e)
  }

  pub fn iter(&self) -> impl Iterator<Item = Entity> + '_ {
    self.entities.iter().copied()
  }
}

impl<C> TrackEntitiesWithComponent<C>
where
  C: Component + DeserializeOwned,
{
  pub fn on_create(&mut self, entity: Entity) {
    if !self.entities.insert(entity) {
      panic!(
        "tried to insert {:x} in a create callback but it was already there",
        entity
      );
    }
  }
  pub fn on_remove(&mut self, entity: Entity) {
    if !self.entities.remove(&entity) {
      panic!(
        "tried to remove {:x} in a create callback but it was not there",
        entity
      );
    }
  }
}

impl<C> Default for TrackEntitiesWithComponent<C>
where
  C: Component,
{
  fn default() -> Self {
    Self::new()
  }
}
