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

    // Get tuple of Refs/RefMuts to components specified by query type
    // (Return tuple of individual options rather than option of complete tuple?... This would
    // make it much more annoying to get the refs out of the result after performing the
    // query. Maybe it should be saved for when I implement explicitly optional components in
    // the query, as in `::<(&mut Position, &Velocity, Option<&Collision>)>`)
    pub fn borrow_components<'ent, Q: ComponentBorrowQuery>(
        &'ent self,
    ) -> Option<Q::Result<'ent>> {
        Q::get_components(&self)
    }

    pub fn add_component<C: Component + 'static>(&mut self, component: C) {
        self.components.insert(RefCell::new(component));
    }

    pub fn remove_component<C: Component + 'static>(&mut self) {
        self.components.remove::<RefCell<C>>();
    }
}

pub struct ECS {
    // Slotmap or similar rather than string ids?
    // Entities could have a name component to refer to them by name in scripts
    // Performance doesn't matter. Just .find() the one with the right name
    // If performance ever mattered, just use a separate index of name to entity id?
    // Such an index could also enforce that names are unique, but for now
    // unchecked is fine
    pub entities: HashMap<String, Entity>,
}

impl ECS {
    // Get iterator over entities that have components specified by query type
    pub fn filter_entities<'ecs, Q: EntityFilterQuery>(
        &'ecs self,
    ) -> Box<dyn Iterator<Item = &'ecs Entity> + 'ecs> {
        Q::filter(self.entities.values())
    }

    // Get iterator over tuples of Refs/RefMuts to components specified by query type for each
    // entity that has those components
    pub fn query<'ecs, Q: ComponentBorrowQuery>(
        &'ecs self,
    ) -> Box<dyn Iterator<Item = Q::Result<'ecs>> + 'ecs> {
        Box::new(
            Q::ToEntityFilterQuery::filter(self.entities.values())
                .map(|e| e.borrow_components::<Q>().unwrap()),
        )
    }

    // Get tuple of Refs/RefMuts to components specified by query type for the specific entity
    // specified by id
    pub fn query_one<'ecs, Q: ComponentBorrowQuery>(
        &'ecs self,
        id: &str,
    ) -> Option<Q::Result<'ecs>> {
        Some(self.entities.get(id)?.borrow_components::<Q>()?)
    }
}

pub trait ComponentBorrowQuery {
    type Result<'res>;
    type ToEntityFilterQuery: EntityFilterQuery;

    fn get_components<'ent>(e: &'ent Entity) -> Option<Self::Result<'ent>>;
}

impl<A> ComponentBorrowQuery for A
where
    A: QueryRef + 'static,
{
    type Result<'res> = A::ResultRef<'res>;
    type ToEntityFilterQuery = A::Component;

    fn get_components<'ent>(e: &'ent Entity) -> Option<Self::Result<'ent>> {
        e.components.get::<RefCell<A::Component>>().map(|rfc| A::borrow(rfc))
    }
}

macro_rules! impl_component_borrow_query_for_tuple {
    ($($name:ident)*) => {
        #[allow(unused)]
        impl<$($name,)*> ComponentBorrowQuery for ($($name,)*)
        where $($name: QueryRef + 'static,)*
        {
            type Result<'res> = ($($name::ResultRef<'res>,)*);
            type ToEntityFilterQuery = ($($name::Component,)*);
            fn get_components<'ent>(e: &'ent Entity) -> Option<Self::Result<'ent>> {
                Some((
                    $(
                        e.components.get::<RefCell<$name::Component>>()
                            .map(|rfc| $name::borrow(rfc))?,
                    )*
                ))
            }
        }
    };
}

impl_component_borrow_query_for_tuple!();
impl_component_borrow_query_for_tuple!(A);
impl_component_borrow_query_for_tuple!(A B);
impl_component_borrow_query_for_tuple!(A B C);
impl_component_borrow_query_for_tuple!(A B C D);
impl_component_borrow_query_for_tuple!(A B C D E);

pub trait QueryRef {
    type Component: Component + 'static;
    type ResultRef<'res>;

    fn borrow<'rfc>(refcell: &'rfc RefCell<Self::Component>) -> Self::ResultRef<'rfc>;
}

impl<C> QueryRef for &C
where
    C: Component + 'static,
{
    type Component = C;
    type ResultRef<'res> = Ref<'res, C>;

    fn borrow<'rfc>(refcell: &'rfc RefCell<Self::Component>) -> Self::ResultRef<'rfc> {
        refcell.borrow()
    }
}

impl<C> QueryRef for &mut C
where
    C: Component + 'static,
{
    type Component = C;
    type ResultRef<'res> = RefMut<'res, C>;

    fn borrow<'rfc>(refcell: &'rfc RefCell<Self::Component>) -> Self::ResultRef<'rfc> {
        refcell.borrow_mut()
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

macro_rules! impl_entity_filter_query_for_tuple {
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

impl_entity_filter_query_for_tuple!();
impl_entity_filter_query_for_tuple!(A);
impl_entity_filter_query_for_tuple!(A B);
impl_entity_filter_query_for_tuple!(A B C);
impl_entity_filter_query_for_tuple!(A B C D);
impl_entity_filter_query_for_tuple!(A B C D E);
