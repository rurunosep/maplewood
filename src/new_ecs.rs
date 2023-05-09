// TODO: hook up and replace old ecs
// TODO: implement QueryRef for &/&mut (but still returning Ref/RefMut) for easier calling?

use anymap::AnyMap;
use std::cell::{Ref, RefCell, RefMut};
use std::collections::HashMap;

pub trait Component {}

pub struct Entity {
    components: AnyMap,
}

impl Entity {
    pub fn new() -> Self {
        Self { components: AnyMap::new() }
    }

    /// Get components specified by query type
    /// Panics if any such components don't exist or are illegally borrowed
    pub fn get_components<'ent, Q: ComponentsQuery>(&'ent self) -> Q::Result<'ent> {
        Q::get_components(&self)
    }

    pub fn add_component<C: Component + 'static>(&mut self, component: C) {
        self.components.insert(RefCell::new(component));
    }

    #[allow(dead_code)]
    pub fn remove_component<C: Component + 'static>(&mut self) {
        self.components.remove::<RefCell<C>>();
    }
}

pub struct ECS {
    pub entities: HashMap<String, Entity>,
}

impl ECS {
    /// Get iterator over entities that have components specified by query type
    pub fn filter_entities<'ecs, Q: EntityFilterQuery>(
        &'ecs self,
    ) -> Box<dyn Iterator<Item = &'ecs Entity> + 'ecs> {
        Q::filter(self.entities.values())
    }

    /// Get iterator over groups of Refs/RefMuts to components specified by query type for each
    /// entity that has those components
    /// Panics if component is borrowed illegally
    pub fn get_components<'ecs, Q: ComponentsQuery>(
        &'ecs self,
    ) -> Box<dyn Iterator<Item = Q::Result<'ecs>> + 'ecs> {
        Box::new(
            Q::ToEntityFilterQuery::filter(self.entities.values())
                .map(|e| e.get_components::<Q>()),
        )
    }
}

pub trait EntityFilterQuery {
    fn filter<'iter>(
        iter: impl Iterator<Item = &'iter Entity> + 'iter,
    ) -> Box<dyn Iterator<Item = &'iter Entity> + 'iter>;
}

impl<A> EntityFilterQuery for A
where
    A: Component + 'static,
{
    fn filter<'iter>(
        iter: impl Iterator<Item = &'iter Entity> + 'iter,
    ) -> Box<dyn Iterator<Item = &'iter Entity> + 'iter> {
        Box::new(iter.filter(|e| e.components.get::<RefCell<A>>().is_some()))
    }
}

// Sample macro expansion for impl_entity_filter_query_tuple!(A B)
//
// impl<A, B> EntityFilterQuery for (A, B)
// where
//     A: Component + 'static,
//     B: Component + 'static,
// {
//     fn filter<'iter>(
//         iter: impl Iterator<Item = &'iter Entity> + 'iter,
//     ) -> Box<dyn Iterator<Item = &'iter Entity> + 'iter> {
//         Box::new(iter.filter(|e| {
//             match (e.components.get::<RefCell<A>>(), e.components.get::<RefCell<B>>()) {
//                 (Some(_), Some(_)) => true,
//                 _ => false,
//             }
//         }))
//     }
// }
macro_rules! impl_entity_filter_query_tuple {
    ($($name:ident)*) => {
        #[allow(unused)]
        #[allow(unreachable_patterns)]
        impl<$($name,)*> EntityFilterQuery for ($($name,)*)
        where $($name: Component + 'static,)*
        {
            fn filter<'iter>(
                iter: impl Iterator<Item = &'iter Entity> + 'iter,
            ) -> Box<dyn Iterator<Item = &'iter Entity> + 'iter> {
                Box::new(iter.filter(|e| {
                    match ($(e.components.get::<RefCell<$name>>(),)*) {
                        (
                            $(
                                replace_pat!($name Some(_))
                            ,)*
                        ) => true,
                        _ => false,
                    }
                }))
            }
        }
    };
}

macro_rules! replace_pat {
    ($_t:tt $sub:pat) => {
        $sub
    };
}

impl_entity_filter_query_tuple!();
impl_entity_filter_query_tuple!(A);
impl_entity_filter_query_tuple!(A B);
impl_entity_filter_query_tuple!(A B C);
impl_entity_filter_query_tuple!(A B C D);
impl_entity_filter_query_tuple!(A B C D E);
impl_entity_filter_query_tuple!(A B C D E F);
impl_entity_filter_query_tuple!(A B C D E F G);
impl_entity_filter_query_tuple!(A B C D E F G H);
impl_entity_filter_query_tuple!(A B C D E F G H I);
impl_entity_filter_query_tuple!(A B C D E F G H I J);

pub trait ComponentsQuery {
    type Result<'res>;
    type ToEntityFilterQuery: EntityFilterQuery;

    fn get_components<'ent>(e: &'ent Entity) -> Self::Result<'ent>;
}

impl<A> ComponentsQuery for A
where
    A: QueryRef + 'static,
{
    type Result<'res> = A::ResultRef<'res>;
    type ToEntityFilterQuery = A::Component;

    fn get_components<'ent>(e: &'ent Entity) -> Self::Result<'ent> {
        A::borrow(e.components.get::<RefCell<A::Component>>().unwrap())
    }
}

// Sample macro expansion for impl_components_query_tuple!(A B):
//
// impl<A, B> ComponentsQuery for (A, B)
// where
//     A: QueryRef + 'static,
//     B: QueryRef + 'static,
// {
//     type Result<'res> = (A::ResultRef<'res>, B::ResultRef<'res>);
//     fn get_components<'ent>(e: &'ent Entity) -> Self::Result<'ent> {
//         (
//             A::borrow(e.components.get::<RefCell<A::Component>>().unwrap()),
//             B::borrow(e.components.get::<RefCell<B::Component>>().unwrap()),
//         )
//     }
// }
macro_rules! impl_components_query_tuple {
    ($($name:ident)*) => {
        #[allow(unused)]
        impl<$($name,)*> ComponentsQuery for ($($name,)*)
        where $($name: QueryRef + 'static,)*
        {
            type Result<'res> = ($($name::ResultRef<'res>,)*);
            type ToEntityFilterQuery = ($($name::Component,)*);
            fn get_components<'ent>(e: &'ent Entity) -> Self::Result<'ent> {
                (
                    $(
                        $name::borrow(e.components.get::<RefCell<$name::Component>>().unwrap()),
                    )*
                )
            }
        }
    };
}

impl_components_query_tuple!();
impl_components_query_tuple!(A);
impl_components_query_tuple!(A B);
impl_components_query_tuple!(A B C);
impl_components_query_tuple!(A B C D);
impl_components_query_tuple!(A B C D E);
impl_components_query_tuple!(A B C D E F);
impl_components_query_tuple!(A B C D E F G);
impl_components_query_tuple!(A B C D E F G H);
impl_components_query_tuple!(A B C D E F G H I);
impl_components_query_tuple!(A B C D E F G H I J);

pub trait QueryRef {
    type Component: Component + 'static;
    type ResultRef<'res>;

    fn borrow<'rfc>(refcell: &'rfc RefCell<Self::Component>) -> Self::ResultRef<'rfc>;
}

impl<C> QueryRef for Ref<'_, C>
where
    C: Component + 'static,
{
    type Component = C;
    type ResultRef<'res> = Ref<'res, C>;

    fn borrow<'rfc>(refcell: &'rfc RefCell<Self::Component>) -> Self::ResultRef<'rfc> {
        refcell.borrow()
    }
}
impl<C> QueryRef for RefMut<'_, C>
where
    C: Component + 'static,
{
    type Component = C;
    type ResultRef<'res> = RefMut<'res, C>;

    fn borrow<'rfc>(refcell: &'rfc RefCell<Self::Component>) -> Self::ResultRef<'rfc> {
        refcell.borrow_mut()
    }
}
