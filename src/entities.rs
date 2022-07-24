use std::collections::BTreeMap;
use std::sync::RwLock;

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
    components: BTreeMap<TypeIdWrapper, (u64, ComponentEntry)>,
    /// This is stored in priority order, where 0 is highest priority
    priorities: BTreeMap<u64, TypeIdWrapper>,
}

impl EntityAssoc {
    pub(crate) fn new() -> Self {
        Self {
            components: BTreeMap::new(),
            priorities: BTreeMap::new(),
        }
    }

    pub(crate) fn insert<C: Component>(&mut self, component: C) -> Option<C> {
        self.insert_boxed(Box::new(component) as _, C::priority())
            .map(|comp| *comp.downcast().unwrap())
    }

    pub(crate) fn insert_boxed(
        &mut self,
        component: Box<dyn Component>,
        priority: u64,
    ) -> Option<Box<dyn Component>> {
        let tid = (*component).type_id_wrapper();
        if let Some(prev) = self.components.get_mut(&tid) {
            let (_, prev) = std::mem::replace(prev, (priority, RwLock::new(component)));
            let prev = prev.into_inner().unwrap();
            Some(prev)
        } else {
            let prev_tid = self.priorities.insert(priority, tid);
            if let Some(prev_tid) = prev_tid {
                panic!("when inserting a component of type {:?}, found a component of type {:?} with the same priority ({})", 
                    tid.type_name, prev_tid.type_name, priority);
            }
            let prev_comp = self
                .components
                .insert(tid, (priority, RwLock::new(component)));
            debug_assert!(prev_comp.is_none());
            None
        }
    }

    pub(crate) fn remove<C: Component>(&mut self) -> Option<C> {
        self.remove_from_tid(TypeIdWrapper::of::<C>())
            .map(|comp| *comp.downcast().unwrap())
    }

    pub(crate) fn remove_from_tid(&mut self, tid: TypeIdWrapper) -> Option<Box<dyn Component>> {
        if let Some((priority, prev)) = self.components.remove(&tid) {
            let prev_tid = self.priorities.remove(&priority);
            debug_assert!(prev_tid.is_some());
            Some(prev.into_inner().unwrap())
        } else {
            None
        }
    }

    pub(crate) fn components(&self) -> &BTreeMap<TypeIdWrapper, (u64, ComponentEntry)> {
        &self.components
    }

    /// Iterate in increasing order of priority
    pub(crate) fn iter(&self) -> impl Iterator<Item = &ComponentEntry> + '_ {
        self.priorities
            .values()
            .map(|tid| &self.components.get(tid).unwrap().1)
    }

    pub(crate) fn len(&self) -> usize {
        self.components.len()
    }
}

/// How each component is stored. Right now this uses naive locking; in the future we might
/// do something fancier.
pub(crate) type ComponentEntry = RwLock<Box<dyn Component>>;
