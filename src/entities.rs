//! Basically the [`generational_arena`](https://docs.rs/generational-arena/0.2.8/generational_arena/) crate, but with
//! some things exposed for internal use, heh.

use std::iter::{self, FusedIterator};
use std::sync::RwLock;
use std::{slice, vec};

use crate::prelude::Component;

/// A handle to a list of [`Component`]s.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Entity {
    pub(crate) index: usize,
    pub(crate) generation: u64,
}

/// Data stored under each entity.
pub(crate) type EntityAssoc = Vec<ComponentEntry>;
/// How each component is stored. Right now this uses naive locking; in the future we might
/// do something fancier.
pub(crate) type ComponentEntry = RwLock<Box<dyn Component>>;

// this doesn't *really* need to be generic cause it's always gonna have EntityAssoc in it but w/e
pub(crate) struct EntityAllocator<T> {
    // Best I can tell, this defines a skip list, like malloc does.
    // `free_list_head` is the entrypoint.
    items: Vec<Entry<T>>,
    generation: u64,
    free_list_head: Option<usize>,
    len: usize,
}

#[derive(Clone, Debug)]
enum Entry<T> {
    Free { next_free: Option<usize> },
    Occupied { generation: u64, value: T },
}

impl<T> EntityAllocator<T> {
    const DEFAULT_CAPACITY: usize = 4;

    pub fn new() -> Self {
        EntityAllocator::with_capacity(Self::DEFAULT_CAPACITY)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        let mut ea = EntityAllocator {
            items: Vec::new(),
            generation: 0,
            free_list_head: None,
            len: 0,
        };
        ea.reserve(capacity);
        ea
    }

    pub fn get(&self, e: Entity) -> Option<&T> {
        match self.items.get(e.index) {
            // If the pattern matches, but not the condition, we have a leftover, dead entity.
            // I'm not sure in what case we would beat the pattern but not the condition?
            // But that's what generational_arena does, beats me.
            Some(Entry::Occupied { generation, value }) if *generation == e.generation => {
                Some(value)
            }
            _ => None,
        }
    }

    pub fn get_mut(&mut self, e: Entity) -> Option<&mut T> {
        match self.items.get_mut(e.index) {
            Some(Entry::Occupied { generation, value }) if *generation == e.generation => {
                Some(value)
            }
            _ => None,
        }
    }

    pub fn try_insert(&mut self, value: T) -> Result<Entity, T> {
        match self.try_alloc_next_index() {
            None => Err(value),
            Some(entity) => {
                self.items[entity.index] = Entry::Occupied {
                    generation: self.generation,
                    value,
                };
                Ok(entity)
            }
        }
    }

    pub fn insert(&mut self, val: T) -> Entity {
        match self.try_insert(val) {
            Ok(e) => e,
            Err(val) => {
                // Like a vec, double the length!
                self.reserve(self.len);
                self.try_insert(val)
                    .map_err(|_| ())
                    .expect("just did an allocation")
            }
        }
    }

    /// Spawn a new entity while never messing with the free list.
    pub fn insert_increasing(&mut self, value: T) -> Entity {
        let index = self.items.len();
        self.items.push(Entry::Occupied {
            generation: self.generation,
            value,
        });
        self.len += 1;
        Entity {
            index,
            generation: self.generation,
        }
    }

    pub fn remove(&mut self, e: Entity) -> Option<T> {
        if e.index >= self.items.len() {
            return None;
        }

        match self.items[e.index] {
            Entry::Occupied { generation, .. } if e.generation == generation => {
                // Append this to the front of the skip list
                let entry = std::mem::replace(
                    &mut self.items[e.index],
                    Entry::Free {
                        next_free: self.free_list_head,
                    },
                );
                self.free_list_head = Some(e.index);

                self.generation += 1;
                self.len -= 1;

                match entry {
                    Entry::Occupied {
                        generation: _,
                        value,
                    } => Some(value),
                    _ => unreachable!(),
                }
            }
            _ => None,
        }
    }

    pub fn reserve(&mut self, additional_capacity: usize) {
        let start = self.items.len();
        let end = start + additional_capacity;
        let old_head = self.free_list_head;
        self.items.reserve_exact(additional_capacity);

        // We make the list point *backwards*, to places before it.
        // Each fresh entry points to the one after it, but the very last entry
        // points to whatever used to be first in the skip list.
        self.items.extend((start..end).map(|i| {
            if i == end - 1 {
                Entry::Free {
                    next_free: old_head,
                }
            } else {
                Entry::Free {
                    next_free: Some(i + 1),
                }
            }
        }));
        self.free_list_head = Some(start);
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn capacity(&self) -> usize {
        self.items.len()
    }

    fn try_alloc_next_index(&mut self) -> Option<Entity> {
        match self.free_list_head {
            None => None,
            Some(i) => match self.items[i] {
                Entry::Occupied { .. } => panic!("corrupt free list"),
                Entry::Free { next_free } => {
                    // pop the car off the list
                    self.free_list_head = next_free;
                    self.len += 1;
                    Some(Entity {
                        index: i,
                        generation: self.generation,
                    })
                }
            },
        }
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn iter(&self) -> Iter<T> {
        Iter {
            len: self.len,
            inner: self.items.iter().enumerate(),
        }
    }

    pub fn iter_mut(&mut self) -> IterMut<T> {
        IterMut {
            len: self.len,
            inner: self.items.iter_mut().enumerate(),
        }
    }
}

impl<T> Default for EntityAllocator<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> IntoIterator for EntityAllocator<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        IntoIter {
            len: self.len,
            inner: self.items.into_iter(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct IntoIter<T> {
    len: usize,
    inner: vec::IntoIter<Entry<T>>,
}

impl<T> Iterator for IntoIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.inner.next() {
                Some(Entry::Free { .. }) => continue,
                Some(Entry::Occupied { value, .. }) => {
                    self.len -= 1;
                    return Some(value);
                }
                None => {
                    debug_assert_eq!(self.len, 0);
                    return None;
                }
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl<T> DoubleEndedIterator for IntoIter<T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        loop {
            match self.inner.next_back() {
                Some(Entry::Free { .. }) => continue,
                Some(Entry::Occupied { value, .. }) => {
                    self.len -= 1;
                    return Some(value);
                }
                None => {
                    debug_assert_eq!(self.len, 0);
                    return None;
                }
            }
        }
    }
}

impl<T> ExactSizeIterator for IntoIter<T> {
    fn len(&self) -> usize {
        self.len
    }
}

impl<T> FusedIterator for IntoIter<T> {}

impl<'a, T> IntoIterator for &'a EntityAllocator<T> {
    type Item = (Entity, &'a T);
    type IntoIter = Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[derive(Clone, Debug)]
pub struct Iter<'a, T: 'a> {
    len: usize,
    inner: iter::Enumerate<slice::Iter<'a, Entry<T>>>,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = (Entity, &'a T);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.inner.next() {
                Some((_, &Entry::Free { .. })) => continue,
                Some((
                    index,
                    &Entry::Occupied {
                        generation,
                        ref value,
                    },
                )) => {
                    self.len -= 1;
                    let idx = Entity { index, generation };
                    return Some((idx, value));
                }
                None => {
                    debug_assert_eq!(self.len, 0);
                    return None;
                }
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl<'a, T> DoubleEndedIterator for Iter<'a, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        loop {
            match self.inner.next_back() {
                Some((_, &Entry::Free { .. })) => continue,
                Some((
                    index,
                    &Entry::Occupied {
                        generation,
                        ref value,
                    },
                )) => {
                    self.len -= 1;
                    let idx = Entity { index, generation };
                    return Some((idx, value));
                }
                None => {
                    debug_assert_eq!(self.len, 0);
                    return None;
                }
            }
        }
    }
}

impl<'a, T> ExactSizeIterator for Iter<'a, T> {
    fn len(&self) -> usize {
        self.len
    }
}

impl<'a, T> FusedIterator for Iter<'a, T> {}

impl<'a, T> IntoIterator for &'a mut EntityAllocator<T> {
    type Item = (Entity, &'a mut T);
    type IntoIter = IterMut<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

#[derive(Debug)]
pub struct IterMut<'a, T: 'a> {
    len: usize,
    inner: iter::Enumerate<slice::IterMut<'a, Entry<T>>>,
}

impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = (Entity, &'a mut T);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.inner.next() {
                Some((_, &mut Entry::Free { .. })) => continue,
                Some((
                    index,
                    &mut Entry::Occupied {
                        generation,
                        ref mut value,
                    },
                )) => {
                    self.len -= 1;
                    let idx = Entity { index, generation };
                    return Some((idx, value));
                }
                None => {
                    debug_assert_eq!(self.len, 0);
                    return None;
                }
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl<'a, T> DoubleEndedIterator for IterMut<'a, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        loop {
            match self.inner.next_back() {
                Some((_, &mut Entry::Free { .. })) => continue,
                Some((
                    index,
                    &mut Entry::Occupied {
                        generation,
                        ref mut value,
                    },
                )) => {
                    self.len -= 1;
                    let idx = Entity { index, generation };
                    return Some((idx, value));
                }
                None => {
                    debug_assert_eq!(self.len, 0);
                    return None;
                }
            }
        }
    }
}

impl<'a, T> ExactSizeIterator for IterMut<'a, T> {
    fn len(&self) -> usize {
        self.len
    }
}

impl<'a, T> FusedIterator for IterMut<'a, T> {}
