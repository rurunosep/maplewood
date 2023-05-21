use crate::components::Label;
use anymap::AnyMap;
use slotmap::{new_key_type, SlotMap};
use std::cell::{Ref, RefCell, RefMut};

pub struct Entity {
    pub id: EntityId,
    pub components: AnyMap,
}

impl Entity {
    pub fn new(id: EntityId) -> Self {
        Self { id, components: AnyMap::new() }
    }

    pub fn borrow<Q>(&self) -> Q::Result<'_>
    where
        Q: Query,
    {
        Q::borrow(self)
    }

    pub fn add_components<B>(&mut self, components: B)
    where
        B: ComponentBundle + 'static,
    {
        components.add(self);
    }

    pub fn remove_components<B>(&mut self)
    where
        B: ComponentBundle + 'static,
    {
        B::remove(self);
    }
}

new_key_type! { pub struct EntityId; }

pub struct Ecs {
    pub entities: SlotMap<EntityId, Entity>,
}

type EntityIter<'a> = Box<dyn Iterator<Item = &'a Entity> + 'a>;
type QueryResultIter<'a, Q> = Box<dyn Iterator<Item = <Q as Query>::Result<'a>> + 'a>;

impl Ecs {
    pub fn new() -> Self {
        Self { entities: SlotMap::with_key() }
    }

    pub fn filter<Q>(&self) -> EntityIter
    where
        Q: Query,
    {
        Box::new(self.entities.values().filter(|e| Q::filter(e)))
    }

    pub fn query_all<Q>(&self) -> QueryResultIter<Q>
    where
        Q: Query,
    {
        Box::new(self.filter::<Q>().map(|e| Q::borrow(e)))
    }

    pub fn query_all_except<Q>(&self, except: EntityId) -> QueryResultIter<Q>
    where
        Q: Query,
    {
        Box::new(
            self.filter::<Q>()
                .filter(move |e| e.borrow::<EntityId>() != except)
                .map(|e| Q::borrow(e)),
        )
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

    pub fn apply_commands(&mut self, commands: EcsCommands) {
        for f in commands.0 {
            f(self)
        }
    }
}

pub struct EcsCommands(Vec<Box<dyn FnOnce(&mut Ecs)>>);

#[allow(dead_code)]
impl EcsCommands {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn add_entity<C>(&mut self, components: C)
    where
        C: ComponentBundle + 'static,
    {
        let f = move |ecs: &mut Ecs| {
            let id = ecs.entities.insert_with_key(|id| Entity::new(id));
            let mut e = ecs.entities.get_mut(id).unwrap();
            components.add(&mut e);
        };
        self.0.push(Box::new(f));
    }

    pub fn remove_entity(&mut self, entity_id: EntityId) {
        let f = move |ecs: &mut Ecs| {
            ecs.entities.remove(entity_id);
        };
        self.0.push(Box::new(f));
    }

    pub fn add_components<B>(&mut self, components: B, entity_id: EntityId)
    where
        B: ComponentBundle + 'static,
    {
        let f = move |ecs: &mut Ecs| {
            ecs.entities.get_mut(entity_id).map(|e| e.add_components(components));
        };
        self.0.push(Box::new(f));
    }

    pub fn remove_components<B>(&mut self, entity_id: EntityId)
    where
        B: Component + 'static,
    {
        let f = move |ecs: &mut Ecs| {
            ecs.entities.get_mut(entity_id).map(|e| e.remove_components::<B>());
        };
        self.0.push(Box::new(f));
    }
}

pub trait Component {}

pub trait ComponentBundle {
    fn add(self, e: &mut Entity);
    fn remove(e: &mut Entity);
}

impl<C> ComponentBundle for C
where
    C: Component + 'static,
{
    fn add(self, e: &mut Entity) {
        e.components.insert(RefCell::new(self));
    }

    fn remove(e: &mut Entity) {
        e.components.remove::<RefCell<C>>();
    }
}

macro_rules! replace_expr {
    ($_t:tt $sub:expr) => {
        $sub
    };
}

macro_rules! impl_component_bundle_for_tuple {
    ($($name:ident)*) => {
        #[allow(unused)]
        impl<$($name,)*> ComponentBundle for ($($name,)*)
        where $($name: Component + 'static,)*
        {
            fn add(self, e: &mut Entity) {
                $(replace_expr!($name e.components.insert(RefCell::new(self.${index()})));)*
            }

            fn remove(e: &mut Entity) {
                $(e.components.remove::<RefCell<$name>>();)*
            }
        }
    };
}

impl_component_bundle_for_tuple!();
impl_component_bundle_for_tuple!(A);
impl_component_bundle_for_tuple!(A B);
impl_component_bundle_for_tuple!(A B C);
impl_component_bundle_for_tuple!(A B C D);
impl_component_bundle_for_tuple!(A B C D E);

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
    type Result<'r> = RefMut<'r, C>;

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
    type Result<'r> = Option<Q::Result<'r>>;

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

macro_rules! impl_query_for_tuple {
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
                    ($(replace_expr!($name true),)*) => true,
                    _ => false,
                }
            }
        }
    };
}

impl_query_for_tuple!();
impl_query_for_tuple!(A);
impl_query_for_tuple!(A B);
impl_query_for_tuple!(A B C);
impl_query_for_tuple!(A B C D);
impl_query_for_tuple!(A B C D E);
