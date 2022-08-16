//! Get components off of entities directly, in a more lightweight way than message passing.

use std::marker::PhantomData;
use std::sync::{RwLockReadGuard, RwLockWriteGuard};

use crate::entities::EntityAssoc;
use crate::prelude::{Component, Entity};
use crate::TypeIdWrapper;

/// Trait for things that can be used to access components.
///
/// This returns `Some` when the query succeeds (finds what it's looking for), and
/// `None` when it doesn't.
///
/// You can query with a `&T` or `&mut T` where `T: Component`.
/// You can also query with `Option<Q> where Q: Query` to get a query that always "succeeds",
/// returning `Some(Some(it))` if it finds the thing and `Some(None)` if it doesn't.
///
/// And finally, you can AND queries by querying for a tuple of `(Q1, Q2, ...)` up to 10 query types.
/// If you need more for some reason, just nest tuples.
///
/// The `'c` lifetime is the lifetime of the references to the components.
///
/// The details of this trait are a private implementation detail (there's nothing sneaky going on,
/// it just depends on internals of the crate I'm planning to change a lot).
pub trait Query<'c> {
    type Response: 'c;
    #[doc(hidden)]
    fn query(entity: Entity, components: &'c EntityAssoc) -> Option<Self::Response>;
}

impl<'c, C: Component> Query<'c> for &'c C {
    type Response = ReadQueryResponse<'c, C>;
    fn query(entity: Entity, components: &'c EntityAssoc) -> Option<Self::Response> {
        let tid = TypeIdWrapper::of::<C>();
        components.components().get(&tid).map(|comp| {
            let lock = comp.try_read().unwrap_or_else(|_| {
                panic!(
                    "{:?} had a component of type {} read queried when it was mutably borrowed",
                    entity, tid.type_name,
                )
            });
            ReadQueryResponse(lock, PhantomData)
        })
    }
}

impl<'c, C: Component> Query<'c> for &'c mut C {
    type Response = WriteQueryResponse<'c, C>;
    fn query(entity: Entity, components: &'c EntityAssoc) -> Option<Self::Response> {
        let tid = TypeIdWrapper::of::<C>();
        components.components().get(&tid).map(|comp| {
            let lock = comp.try_write().unwrap_or_else(|_| {
                panic!(
                    "{:?} had a component of type {} write queried when it was borrowed",
                    entity, tid.type_name
                )
            });
            WriteQueryResponse(lock, PhantomData)
        })
    }
}

impl<'c, Q: Query<'c>> Query<'c> for Option<Q> {
    type Response = Option<Q::Response>;
    fn query(entity: Entity, components: &'c EntityAssoc) -> Option<Self::Response> {
        Some(Q::query(entity, components))
    }
}

macro_rules! impl_query {
    ($($subquery:ident),*) => {
        #[allow(non_snake_case)]
        impl<'c, $($subquery,)*> Query<'c> for ($($subquery,)*)
            where $($subquery: Query<'c>,)*
        {
            type Response = ($(<$subquery as Query<'c>>::Response,)*);

            fn query(entity: Entity, components: &'c EntityAssoc) -> Option<Self::Response> {
                Some((
                    $($subquery::query(entity, components)?,)*
                ))
            }
        }
    };
}

impl_query!(A);
impl_query!(A, B);
impl_query!(A, B, C);
impl_query!(A, B, C, D);
impl_query!(A, B, C, D, E);
impl_query!(A, B, C, D, E, F);
impl_query!(A, B, C, D, E, F, G);
impl_query!(A, B, C, D, E, F, G, H);
impl_query!(A, B, C, D, E, F, G, H, I);
impl_query!(A, B, C, D, E, F, G, H, I, J);

/// Wrapper struct returned when querying `&T`
pub struct ReadQueryResponse<'a, T>(RwLockReadGuard<'a, Box<dyn Component>>, PhantomData<&'a T>);

impl<T: 'static> std::ops::Deref for ReadQueryResponse<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // SAFETY: we checked that this `is` of the wanted type in the query method.
        unsafe { self.0.downcast_ref().unwrap_unchecked() }
    }
}

impl<T: 'static> AsRef<T> for ReadQueryResponse<'_, T> {
    fn as_ref(&self) -> &T {
        &*self
    }
}

/// Wrapper struct returned when querying `&mut T`
pub struct WriteQueryResponse<'a, T>(
    RwLockWriteGuard<'a, Box<dyn Component>>,
    PhantomData<&'a mut T>,
);

impl<T: 'static> std::ops::Deref for WriteQueryResponse<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // SAFETY: we checked that this `is` of the wanted type in the query method.
        unsafe { self.0.downcast_ref().unwrap_unchecked() }
    }
}

impl<T: 'static> std::ops::DerefMut for WriteQueryResponse<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: we checked that this `is` of the wanted type in the query method.
        unsafe { self.0.downcast_mut().unwrap_unchecked() }
    }
}
