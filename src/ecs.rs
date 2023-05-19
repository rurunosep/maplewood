use crate::components::Label;
use anymap::AnyMap;
use slotmap::{new_key_type, Key, SlotMap};
use std::cell::{Ref, RefCell, RefMut};

pub trait Component {}

pub struct Entity {
    components: AnyMap,
}

impl Entity {
    pub fn new() -> Self {
        Self { components: AnyMap::new() }
    }

    pub fn borrow_components<Q>(&self) -> Q::Result<'_>
    where
        Q: ComponentBorrowQuery,
    {
        Q::get_components((EntityId::null(), self))
    }

    pub fn add_component<C>(&mut self, component: C)
    where
        C: Component + 'static,
    {
        self.components.insert(RefCell::new(component));
    }

    pub fn remove_component<C>(&mut self)
    where
        C: Component + 'static,
    {
        self.components.remove::<RefCell<C>>();
    }
}

new_key_type! { pub struct EntityId; }

pub struct Ecs {
    pub entities: SlotMap<EntityId, Entity>,
}

impl Ecs {
    pub fn filter_entities<Q>(&self) -> Box<dyn Iterator<Item = (EntityId, &Entity)> + '_>
    where
        Q: EntityFilterQuery,
    {
        Box::new(self.entities.iter().filter(|(_, e)| Q::filter(e)))
    }

    pub fn query<Q>(&self) -> Box<dyn Iterator<Item = Q::Result<'_>> + '_>
    where
        Q: ComponentBorrowQuery,
    {
        Box::new(
            self.filter_entities::<Q::ToEntityFilterQuery>()
                .map(|(id, e)| Q::get_components((id, e))),
        )
    }

    pub fn query_one<Q>(&self, id: EntityId) -> Option<Q::Result<'_>>
    where
        Q: ComponentBorrowQuery,
    {
        self.entities
            .get(id)
            .filter(|e| Q::ToEntityFilterQuery::filter(e))
            .map(|e| e.borrow_components::<Q>())
    }

    pub fn find_by_label(&self, label: &str) -> Option<EntityId> {
        self.query::<(EntityId, &Label)>()
            .find(|(_, l)| l.0.as_str() == label)
            .map(|(id, _)| id)
    }
}

pub trait ComponentBorrowQuery {
    type Result<'r>;
    type ToEntityFilterQuery: EntityFilterQuery;

    fn get_components(id_e: (EntityId, &Entity)) -> Self::Result<'_>;
}

impl<A> ComponentBorrowQuery for A
where
    A: BorrowQueryElement + 'static,
{
    type Result<'r> = A::Result<'r>;
    type ToEntityFilterQuery = A::ToFilterQueryElement;

    fn get_components((id, e): (EntityId, &Entity)) -> Self::Result<'_> {
        A::borrow((id, e))
    }
}

macro_rules! impl_component_borrow_query_for_tuple {
    ($($name:ident)*) => {
        #[allow(unused)]
        #[allow(clippy::unused_unit)]
        impl<$($name,)*> ComponentBorrowQuery for ($($name,)*)
        where $($name: BorrowQueryElement + 'static,)*
        {
            type Result<'r> = ($($name::Result<'r>,)*);
            type ToEntityFilterQuery = ($($name::ToFilterQueryElement,)*);

            fn get_components((id, e): (EntityId, &Entity)) -> Self::Result<'_> {
                ($($name::borrow((id, e)),)*)
            }
        }
    };
}

impl_component_borrow_query_for_tuple!(A);
impl_component_borrow_query_for_tuple!(A B);
impl_component_borrow_query_for_tuple!(A B C);
impl_component_borrow_query_for_tuple!(A B C D);
impl_component_borrow_query_for_tuple!(A B C D E);

pub trait BorrowQueryElement {
    type ToFilterQueryElement: FilterQueryElement;
    type Result<'r>;

    fn borrow(id_e: (EntityId, &Entity)) -> Self::Result<'_>;
}

impl BorrowQueryElement for () {
    type ToFilterQueryElement = ();
    type Result<'r> = ();
    fn borrow(_: (EntityId, &Entity)) -> Self::Result<'_> {}
}

impl BorrowQueryElement for EntityId {
    type ToFilterQueryElement = ();
    type Result<'r> = EntityId;

    fn borrow((id, _): (EntityId, &Entity)) -> Self::Result<'_> {
        id
    }
}

impl<C> BorrowQueryElement for &C
where
    C: Component + 'static,
{
    type ToFilterQueryElement = C;
    type Result<'r> = Ref<'r, C>;

    fn borrow((_, e): (EntityId, &Entity)) -> Self::Result<'_> {
        e.components.get::<RefCell<C>>().unwrap().borrow()
    }
}

impl<C> BorrowQueryElement for &mut C
where
    C: Component + 'static,
{
    type ToFilterQueryElement = C;
    type Result<'res> = RefMut<'res, C>;

    fn borrow((_, e): (EntityId, &Entity)) -> Self::Result<'_> {
        e.components.get::<RefCell<C>>().unwrap().borrow_mut()
    }
}

impl<C> BorrowQueryElement for Option<&C>
where
    C: Component + 'static,
{
    type ToFilterQueryElement = ();
    type Result<'res> = Option<Ref<'res, C>>;

    fn borrow((_, e): (EntityId, &Entity)) -> Self::Result<'_> {
        e.components.get::<RefCell<C>>().map(|r| r.borrow())
    }
}

impl<C> BorrowQueryElement for Option<&mut C>
where
    C: Component + 'static,
{
    type ToFilterQueryElement = ();
    type Result<'res> = Option<RefMut<'res, C>>;

    fn borrow((_, e): (EntityId, &Entity)) -> Self::Result<'_> {
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

impl_entity_filter_query_for_tuple!(A);
impl_entity_filter_query_for_tuple!(A B);
impl_entity_filter_query_for_tuple!(A B C);
impl_entity_filter_query_for_tuple!(A B C D);
impl_entity_filter_query_for_tuple!(A B C D E);

pub trait FilterQueryElement {
    fn filter(e: &Entity) -> bool;
}

impl FilterQueryElement for () {
    fn filter(_: &Entity) -> bool {
        true
    }
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
