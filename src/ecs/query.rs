use super::{Component, ComponentMap, EntityId};
use anymap::AnyMap;
use std::cell::{Ref, RefMut};

pub trait Query {
    type Result<'r>;

    fn borrow(id: EntityId, component_maps: &AnyMap) -> Self::Result<'_>;
    fn filter(id: EntityId, component_maps: &AnyMap) -> bool;
}

impl Query for EntityId {
    type Result<'r> = EntityId;

    fn borrow(id: EntityId, _: &AnyMap) -> Self::Result<'_> {
        id
    }

    fn filter(_: EntityId, _: &AnyMap) -> bool {
        true
    }
}

impl<C> Query for &C
where
    C: Component + 'static,
{
    type Result<'r> = Ref<'r, C>;

    fn borrow(id: EntityId, component_maps: &AnyMap) -> Self::Result<'_> {
        component_maps.get::<ComponentMap<C>>().unwrap().get(id).unwrap().borrow()
    }

    fn filter(id: EntityId, component_maps: &AnyMap) -> bool {
        component_maps
            .get::<ComponentMap<C>>()
            .map(|cm| cm.contains_key(id))
            .unwrap_or(false)
    }
}

impl<C> Query for &mut C
where
    C: Component + 'static,
{
    type Result<'r> = RefMut<'r, C>;

    fn borrow(id: EntityId, component_maps: &AnyMap) -> Self::Result<'_> {
        component_maps.get::<ComponentMap<C>>().unwrap().get(id).unwrap().borrow_mut()
    }

    fn filter(id: EntityId, component_maps: &AnyMap) -> bool {
        component_maps
            .get::<ComponentMap<C>>()
            .map(|cm| cm.contains_key(id))
            .unwrap_or(false)
    }
}

impl<Q> Query for Option<Q>
where
    Q: Query + 'static,
{
    type Result<'r> = Option<Q::Result<'r>>;

    fn borrow(id: EntityId, component_maps: &AnyMap) -> Self::Result<'_> {
        if Q::filter(id, component_maps) {
            Some(Q::borrow(id, component_maps))
        } else {
            None
        }
    }

    fn filter(_: EntityId, _: &AnyMap) -> bool {
        true
    }
}

// Are With<C> and Without<C> even necessary?

pub struct With<C>(std::marker::PhantomData<C>)
where
    C: Component + 'static;

impl<C> Query for With<C>
where
    C: Component + 'static,
{
    type Result<'r> = ();

    fn filter(id: EntityId, component_maps: &AnyMap) -> bool {
        component_maps
            .get::<ComponentMap<C>>()
            .map(|cm| cm.contains_key(id))
            .unwrap_or(false)
    }

    fn borrow(_: EntityId, _: &AnyMap) -> Self::Result<'_> {}
}

pub struct Without<C>(std::marker::PhantomData<C>)
where
    C: Component + 'static;

impl<C> Query for Without<C>
where
    C: Component + 'static,
{
    type Result<'r> = ();

    fn filter(id: EntityId, component_maps: &AnyMap) -> bool {
        !component_maps
            .get::<ComponentMap<C>>()
            .map(|cm| cm.contains_key(id))
            .unwrap_or(false)
    }

    fn borrow(_: EntityId, _: &AnyMap) -> Self::Result<'_> {}
}

macro_rules! impl_query_for_tuple {
  ($($name:ident)*) => {
      #[allow(unused)]
      #[allow(clippy::unused_unit)]
      impl<$($name,)*> Query for ($($name,)*)
      where $($name: Query + 'static,)*
      {
          type Result<'r> = ($($name::Result<'r>,)*);

          fn borrow(id: EntityId, component_maps: &AnyMap) -> Self::Result<'_> {
              ($($name::borrow(id, component_maps),)*)
          }

          fn filter(id: EntityId, component_maps: &AnyMap) -> bool {
              match ($($name::filter(id, component_maps),)*) {
                  ($(replace_expr!($name true),)*) => true,
                  _ => false,
              }
          }
      }
  };
}

macro_rules! replace_expr {
    ($_t:tt $repl:expr) => {
        $repl
    };
}

impl_query_for_tuple!();
impl_query_for_tuple!(A);
impl_query_for_tuple!(A B);
impl_query_for_tuple!(A B C);
impl_query_for_tuple!(A B C D);
impl_query_for_tuple!(A B C D E);
