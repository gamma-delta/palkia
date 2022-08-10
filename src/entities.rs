use std::{iter, sync::RwLock};

use indexmap::IndexMap;

use crate::{
    allocator,
    prelude::{Component, World},
};
use crate::{ToTypeIdWrapper, TypeIdWrapper};

/// A handle to a list of [`Component`]s.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Entity {
    pub(crate) index: usize,
    pub(crate) generation: u64,
}

/// Data stored under each entity.
///
/// The internals of this are private and you really shouldn't be using it;
/// I need to make it public for `Query` though.
pub struct EntityAssoc {
    components: IndexMap<TypeIdWrapper, ComponentEntry, ahash::RandomState>,
}

impl EntityAssoc {
    pub(crate) fn new(components: impl IntoIterator<Item = Box<dyn Component>>) -> Self {
        let components = components
            .into_iter()
            .map(|comp| ((*comp).type_id_wrapper(), RwLock::new(comp)))
            .collect();
        Self { components }
    }

    pub(crate) fn empty() -> Self {
        Self {
            components: IndexMap::default(),
        }
    }

    /// Iterate in increasing order of priority
    pub(crate) fn iter(&self) -> impl Iterator<Item = (TypeIdWrapper, &ComponentEntry)> + '_ {
        self.components.iter().map(|(tid, comp)| (*tid, comp))
    }

    pub(crate) fn into_iter(self) -> impl Iterator<Item = (TypeIdWrapper, ComponentEntry)> {
        self.components.into_iter()
    }

    pub(crate) fn len(&self) -> usize {
        self.components.len()
    }

    pub(crate) fn components(
        &self,
    ) -> &IndexMap<TypeIdWrapper, ComponentEntry, ahash::RandomState> {
        &self.components
    }
}

/// Iterator over all the entities in a world, in no particular order.
pub struct EntityIter<'w> {
    iter: iter::Map<allocator::Iter<'w, EntityAssoc>, fn((Entity, &EntityAssoc)) -> Entity>,
}

impl<'w> EntityIter<'w> {
    pub(crate) fn new(w: &'w World) -> Self {
        // Make this a non-closure so we can have a writeable type
        fn car(pair: (Entity, &EntityAssoc)) -> Entity {
            pair.0
        }

        Self {
            iter: w.entities.iter().map(car),
        }
    }
}

impl<'w> Iterator for EntityIter<'w> {
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

/// How each component is stored. Right now this uses naive locking; in the future we might
/// do something fancier.
pub(crate) type ComponentEntry = RwLock<Box<dyn Component>>;
