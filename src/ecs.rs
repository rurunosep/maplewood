use crate::components::Label;
use anymap::AnyMap;
use slotmap::{new_key_type, SlotMap};
use std::cell::{Ref, RefCell, RefMut};

pub trait Component {}

pub struct Entity {
    id: EntityId,
    components: AnyMap,
}

impl Entity {
    pub fn new(id: EntityId) -> Self {
        Self { id, components: AnyMap::new() }
    }

    pub fn borrow_components<Q>(&self) -> Q::Result<'_>
    where
        Q: Query,
    {
        Q::borrow(self)
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
    pub fn filter_entities<Q>(&self) -> Box<dyn Iterator<Item = &Entity> + '_>
    where
        Q: Query,
    {
        Box::new(self.entities.values().filter(|e| Q::filter(e)))
    }

    pub fn query_all<Q>(&self) -> Box<dyn Iterator<Item = Q::Result<'_>> + '_>
    where
        Q: Query,
    {
        Box::new(self.filter_entities::<Q>().map(|e| Q::borrow(e)))
    }

    pub fn query_one_by_id<Q>(&self, id: EntityId) -> Option<Q::Result<'_>>
    where
        Q: Query,
    {
        self.entities.get(id).filter(|e| Q::filter(e)).map(|e| Q::borrow(e))
    }

    pub fn query_one_by_label<Q>(&self, label: &str) -> Option<Q::Result<'_>>
    where
        Q: Query + 'static,
    {
        self.query_all::<(&Label, Q)>().find(|(l, _)| l.0.as_str() == label).map(|(_, q)| q)
    }
}

pub trait Query {
    type Result<'r>;

    fn borrow(e: &Entity) -> Self::Result<'_>;
    fn filter(e: &Entity) -> bool;
}

impl Query for EntityId {
    type Result<'r> = EntityId;

    fn borrow(e: &Entity) -> Self::Result<'_> {
        e.id
    }

    fn filter(_: &Entity) -> bool {
        true
    }
}

impl<C> Query for &C
where
    C: Component + 'static,
{
    type Result<'r> = Ref<'r, C>;

    fn borrow(e: &Entity) -> Self::Result<'_> {
        e.components.get::<RefCell<C>>().unwrap().borrow()
    }

    fn filter(e: &Entity) -> bool {
        e.components.get::<RefCell<C>>().is_some()
    }
}

impl<C> Query for &mut C
where
    C: Component + 'static,
{
    type Result<'res> = RefMut<'res, C>;

    fn borrow(e: &Entity) -> Self::Result<'_> {
        e.components.get::<RefCell<C>>().unwrap().borrow_mut()
    }

    fn filter(e: &Entity) -> bool {
        e.components.get::<RefCell<C>>().is_some()
    }
}

impl<Q> Query for Option<Q>
where
    Q: Query + 'static,
{
    type Result<'res> = Option<Q::Result<'res>>;

    fn borrow(e: &Entity) -> Self::Result<'_> {
        if Q::filter(e) {
            Some(Q::borrow(e))
        } else {
            None
        }
    }

    fn filter(_: &Entity) -> bool {
        true
    }
}

macro_rules! impl_component_borrow_query_for_tuple {
    ($($name:ident)*) => {
        #[allow(unused)]
        #[allow(clippy::unused_unit)]
        impl<$($name,)*> Query for ($($name,)*)
        where $($name: Query + 'static,)*
        {
            type Result<'r> = ($($name::Result<'r>,)*);

            fn borrow(e: &Entity) -> Self::Result<'_> {
                ($($name::borrow(e),)*)
            }

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

impl_component_borrow_query_for_tuple!();
impl_component_borrow_query_for_tuple!(A);
impl_component_borrow_query_for_tuple!(A B);
impl_component_borrow_query_for_tuple!(A B C);
impl_component_borrow_query_for_tuple!(A B C D);
impl_component_borrow_query_for_tuple!(A B C D E);
