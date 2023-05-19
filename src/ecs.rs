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

    pub fn borrow_components<'ent, Q: ComponentBorrowQuery>(&'ent self) -> Q::Result<'ent> {
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
    pub fn filter_entities<'ecs, Q: EntityFilterQuery>(
        &'ecs self,
    ) -> Box<dyn Iterator<Item = &'ecs Entity> + 'ecs> {
        Box::new(self.entities.values().filter(|e| Q::filter(e)))
    }

    pub fn query<'ecs, Q: ComponentBorrowQuery>(
        &'ecs self,
    ) -> Box<dyn Iterator<Item = Q::Result<'ecs>> + 'ecs> {
        Box::new(
            self.filter_entities::<Q::ToEntityFilterQuery>()
                .map(|e| e.borrow_components::<Q>()),
        )
    }

    pub fn query_one<'ecs, Q: ComponentBorrowQuery>(
        &'ecs self,
        id: &str,
    ) -> Option<Q::Result<'ecs>> {
        self.entities
            .get(id)
            .filter(|e| Q::ToEntityFilterQuery::filter(e))
            .map(|e| e.borrow_components::<Q>())
    }
}

pub trait ComponentBorrowQuery {
    type Result<'res>;
    type ToEntityFilterQuery: EntityFilterQuery;

    fn get_components<'ent>(e: &'ent Entity) -> Self::Result<'ent>;
}

impl<A> ComponentBorrowQuery for A
where
    A: BorrowQueryElement + 'static,
{
    type Result<'res> = A::Result<'res>;
    type ToEntityFilterQuery = A::ToFilterQueryElement;

    fn get_components<'ent>(e: &'ent Entity) -> Self::Result<'ent> {
        A::borrow(e)
    }
}

macro_rules! impl_component_borrow_query_for_tuple {
    ($($name:ident)*) => {
        #[allow(unused)]
        impl<$($name,)*> ComponentBorrowQuery for ($($name,)*)
        where $($name: BorrowQueryElement + 'static,)*
        {
            type Result<'res> = ($($name::Result<'res>,)*);
            type ToEntityFilterQuery = ($($name::ToFilterQueryElement,)*);

            fn get_components<'ent>(e: &'ent Entity) -> Self::Result<'ent> {
                ($($name::borrow(e),)*)
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

pub trait BorrowQueryElement {
    type ToFilterQueryElement: FilterQueryElement;
    type Result<'res>;

    fn borrow<'ent>(e: &'ent Entity) -> Self::Result<'ent>;
}

impl<C> BorrowQueryElement for &C
where
    C: Component + 'static,
{
    type ToFilterQueryElement = C;
    type Result<'res> = Ref<'res, C>;

    fn borrow<'ent>(e: &'ent Entity) -> Self::Result<'ent> {
        // Panics here if the entity doesn't have the component (or if illegal borrow)
        e.components.get::<RefCell<C>>().unwrap().borrow()
    }
}

impl<C> BorrowQueryElement for &mut C
where
    C: Component + 'static,
{
    type ToFilterQueryElement = C;
    type Result<'res> = RefMut<'res, C>;

    fn borrow<'ent>(e: &'ent Entity) -> Self::Result<'ent> {
        e.components.get::<RefCell<C>>().unwrap().borrow_mut()
    }
}

impl<C> BorrowQueryElement for Option<&C>
where
    C: Component + 'static,
{
    type ToFilterQueryElement = Option<C>;
    type Result<'res> = Option<Ref<'res, C>>;

    fn borrow<'ent>(e: &'ent Entity) -> Self::Result<'ent> {
        // Doesn't panic if entity doesn't have component. Returns an option.
        // (Still panics if illegal borrow)
        e.components.get::<RefCell<C>>().map(|r| r.borrow())
    }
}

impl<C> BorrowQueryElement for Option<&mut C>
where
    C: Component + 'static,
{
    type ToFilterQueryElement = Option<C>;
    type Result<'res> = Option<RefMut<'res, C>>;

    fn borrow<'ent>(e: &'ent Entity) -> Self::Result<'ent> {
        e.components.get::<RefCell<C>>().map(|r| r.borrow_mut())
    }
}

pub trait EntityFilterQuery {
    fn filter(e: &Entity) -> bool;
}

impl<A> EntityFilterQuery for A
where
    A: FilterQueryElement + 'static,
{
    fn filter(e: &Entity) -> bool {
        A::filter(e)
    }
}

macro_rules! impl_entity_filter_query_for_tuple {
    ($($name:ident)*) => {
        #[allow(unused)]
        #[allow(unreachable_patterns)]
        impl<$($name,)*> EntityFilterQuery for ($($name,)*)
        where $($name: FilterQueryElement + 'static,)*
        {
            fn filter(e: &Entity) -> bool {
                match ($($name::filter(e),)*) {
                    ($(replace_pat!($name true),)*) => true,
                    _ => false,
                }
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

pub trait FilterQueryElement {
    fn filter(e: &Entity) -> bool;
}

impl<T> FilterQueryElement for T
where
    T: Component + 'static,
{
    fn filter(e: &Entity) -> bool {
        e.components.get::<RefCell<T>>().is_some()
    }
}

impl<T> FilterQueryElement for Option<T>
where
    T: Component + 'static,
{
    fn filter(_: &Entity) -> bool {
        true
    }
}
