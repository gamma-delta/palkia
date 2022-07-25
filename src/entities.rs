use std::sync::RwLock;

use indexmap::IndexMap;

use crate::prelude::Component;
use crate::{ToTypeIdWrapper, TypeIdWrapper};

/// A handle to a list of [`Component`]s.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    pub(crate) fn iter(&self) -> impl Iterator<Item = &ComponentEntry> + '_ {
        self.components.values()
    }

    pub(crate) fn into_iter(self) -> impl Iterator<Item = ComponentEntry> {
        self.components.into_values()
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

/// How each component is stored. Right now this uses naive locking; in the future we might
/// do something fancier.
pub(crate) type ComponentEntry = RwLock<Box<dyn Component>>;
