//! Lightweight handles to lists of resources.

use std::{collections::hash_map, fmt, iter};

use generational_arena::Index;
use serde::{Deserialize, Serialize};

use crate::world::EntityAssoc;

/// A handle to a list of [`Component`]s.
///
/// Often while debugging the whole long-form `Debug` impl is a bit long
/// to print to the screen.
/// The `LowerHex` fmt impl prints it shorter as `index@generation`.
/// (This uses decimal numbers. Yes this isn't what format specifiers
/// are supposed to be for. I don't care.)
#[derive(
  Debug,
  Clone,
  Copy,
  PartialEq,
  Eq,
  Hash,
  PartialOrd,
  Ord,
  Serialize,
  Deserialize,
)]
#[serde(transparent)]
pub struct Entity(pub(crate) Index);

impl Entity {
  /// Decompose an [`Entity`] into its raw parts: `(index, generation)`.
  ///
  /// You almost certainly do NOT want to call this...
  ///
  /// See the [`generational-arena` docs](https://docs.rs/generational-arena/0.2.8/generational_arena/struct.Index.html#implementations)
  /// (which this crate uses internally) for more.
  pub fn decompose(self) -> (usize, u64) {
    self.0.into_raw_parts()
  }

  /// Recompose an [`Entity`] from values you got from
  /// [`decompose`](Entity::decompose).
  ///
  /// Please don't call this from values you didn't get from `decompose`;
  /// it will lead to errors and probably panics, and possibly UB.
  pub fn recompose(index: usize, generation: u64) -> Self {
    Self(Index::from_raw_parts(index, generation))
  }
}

impl fmt::LowerHex for Entity {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let (idx, gen) = self.decompose();
    write!(f, "{}@{}", idx, gen)
  }
}

/// Iterator over all the entities in a world, in no particular order.
pub struct EntityIter<'a> {
  pub(crate) iter: iter::Copied<hash_map::Keys<'a, Entity, EntityAssoc>>,
}

impl<'a> Iterator for EntityIter<'a> {
  type Item = Entity;

  fn next(&mut self) -> Option<Self::Item> {
    self.iter.next()
  }

  fn size_hint(&self) -> (usize, Option<usize>) {
    self.iter.size_hint()
  }
}

impl<'w> ExactSizeIterator for EntityIter<'w> {
  fn len(&self) -> usize {
    self.iter.len()
  }
}

/// The state an entity can be in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityLiveness {
  /// The entity is completely alive; you can do whatever to it.
  Alive,
  /// The entity is dead. Either it's been despawned, or you
  /// [`Entity::recompose`]d something you shouldn't have.
  Dead,
  /// The entity *will* be alive once [`World::finalize`] is called.
  PartiallySpawned,
}
