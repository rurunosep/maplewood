use crate::components::Label;
use anymap::AnyMap;
use slotmap::{new_key_type, Key, SlotMap};
use std::cell::{Ref, RefCell, RefMut};

pub trait Component {}

pub struct Entity {
    pub id: EntityId,
    pub components: AnyMap,
}

impl Entity {
    pub fn new() -> Self {
        Self { id: Key::null(), components: AnyMap::new() }
    }

    #[allow(dead_code)]
    pub fn borrow<Q>(&self) -> Q::Result<'_>
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
        self.components.remove::<C>();
    }
}

new_key_type! { pub struct EntityId; }

pub struct Ecs {
    pub entities: SlotMap<EntityId, Entity>,
    pub deferred_mutations: RefCell<Vec<Box<dyn FnOnce(&mut Ecs)>>>,
}

type EntityIter<'a> = Box<dyn Iterator<Item = &'a Entity> + 'a>;
type QueryResultIter<'a, Q> = Box<dyn Iterator<Item = <Q as Query>::Result<'a>> + 'a>;

impl Ecs {
    pub fn new() -> Self {
        Self { entities: SlotMap::with_key(), deferred_mutations: RefCell::new(Vec::new()) }
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
                .filter(move |e| <EntityId as Query>::borrow(e) != except)
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

    pub fn add_entity(&mut self, mut entity: Entity) -> EntityId {
        self.entities.insert_with_key(|id| {
            entity.id = id;
            entity
        })
    }

    pub fn remove_entity(&mut self, entity_id: EntityId) {
        self.entities.remove(entity_id);
    }

    pub fn add_component<C>(&mut self, entity_id: EntityId, component: C)
    where
        C: Component + 'static,
    {
        self.entities.get_mut(entity_id).map(|e| e.add_component(component));
    }

    pub fn remove_component<C>(&mut self, entity_id: EntityId)
    where
        C: Component + 'static,
    {
        self.entities.get_mut(entity_id).map(|e| e.remove_component::<C>());
    }

    #[allow(dead_code)]
    pub fn add_entity_deferred(&self, entity: Entity) {
        self.deferred_mutations.borrow_mut().push(Box::new(move |ecs: &mut Ecs| {
            ecs.add_entity(entity);
        }));
    }

    #[allow(dead_code)]
    pub fn remove_entity_deferred(&self, entity_id: EntityId) {
        self.deferred_mutations.borrow_mut().push(Box::new(move |ecs: &mut Ecs| {
            ecs.remove_entity(entity_id);
        }));
    }

    #[allow(dead_code)]
    pub fn add_component_deferred<C>(&self, entity_id: EntityId, component: C)
    where
        C: Component + 'static,
    {
        self.deferred_mutations.borrow_mut().push(Box::new(move |ecs: &mut Ecs| {
            ecs.add_component(entity_id, component);
        }));
    }

    #[allow(dead_code)]
    pub fn remove_component_deferred<C>(&self, entity_id: EntityId)
    where
        C: Component + 'static,
    {
        self.deferred_mutations.borrow_mut().push(Box::new(move |ecs: &mut Ecs| {
            ecs.remove_component::<C>(entity_id);
        }));
    }

    pub fn flush_deferred_mutations(&mut self) {
        for f in self.deferred_mutations.take() {
            f(self);
        }
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
