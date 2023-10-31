use crate::components::Label;
use anymap::AnyMap;
use slotmap::{new_key_type, Key, SecondaryMap, SlotMap};
use std::cell::{Ref, RefCell, RefMut};

pub trait Component {}

new_key_type! { pub struct EntityId; }
new_key_type! { pub struct DeferredEntityId; }

pub enum RealOrDeferredEntityId {
    Real(EntityId),
    Deferred(DeferredEntityId),
}

impl From<EntityId> for RealOrDeferredEntityId {
    fn from(id: EntityId) -> Self {
        RealOrDeferredEntityId::Real(id)
    }
}

impl From<DeferredEntityId> for RealOrDeferredEntityId {
    fn from(id: DeferredEntityId) -> Self {
        RealOrDeferredEntityId::Deferred(id)
    }
}

type QueryResultIter<'a, Q> = Box<dyn Iterator<Item = <Q as Query>::Result<'a>> + 'a>;
type ComponentMap<C> = SecondaryMap<EntityId, RefCell<C>>;

pub struct Ecs {
    pub entity_ids: SlotMap<EntityId, ()>,
    pub component_maps: AnyMap,
    pub deferred_mutations: RefCell<Vec<Box<dyn FnOnce(&mut Ecs)>>>,
    pub deferred_entity_ids: RefCell<SlotMap<DeferredEntityId, EntityId>>,
}

impl Ecs {
    pub fn new() -> Self {
        Self {
            entity_ids: SlotMap::with_key(),
            component_maps: AnyMap::new(),
            deferred_mutations: RefCell::new(Vec::new()),
            deferred_entity_ids: RefCell::new(SlotMap::with_key()),
        }
    }

    pub fn filter<Q>(&self) -> Box<dyn Iterator<Item = EntityId> + '_>
    where
        Q: Query,
    {
        Box::new(self.entity_ids.keys().filter(|id| Q::filter(*id, &self.component_maps)))
    }

    pub fn query_all<Q>(&self) -> QueryResultIter<Q>
    where
        Q: Query,
    {
        Box::new(self.filter::<Q>().map(|id| Q::borrow(id, &self.component_maps)))
    }

    pub fn query_all_except<Q>(&self, except: EntityId) -> QueryResultIter<Q>
    where
        Q: Query,
    {
        Box::new(
            self.filter::<Q>()
                .filter(move |id| *id != except)
                .map(|id| Q::borrow(id, &self.component_maps)),
        )
    }

    pub fn query_one_by_id<Q>(&self, id: EntityId) -> Option<Q::Result<'_>>
    where
        Q: Query,
    {
        Some(id)
            .filter(|id| Q::filter(*id, &self.component_maps))
            .map(|id| Q::borrow(id, &self.component_maps))
    }

    pub fn query_one_by_label<Q>(&self, label: &str) -> Option<Q::Result<'_>>
    where
        Q: Query + 'static,
    {
        self.query_all::<(&Label, Q)>().find(|(l, _)| l.0.as_str() == label).map(|(_, q)| q)
    }

    pub fn add_entity(&mut self) -> EntityId {
        self.entity_ids.insert(())
    }

    pub fn remove_entity(&mut self, entity_id: EntityId) {
        self.entity_ids.remove(entity_id);
    }

    pub fn add_component<C>(&mut self, entity_id: EntityId, component: C)
    where
        C: Component + 'static,
    {
        match self.component_maps.get_mut::<ComponentMap<C>>() {
            Some(cm) => {
                cm.insert(entity_id, RefCell::new(component));
            }
            None => {
                let mut cm = SecondaryMap::<EntityId, RefCell<C>>::new();
                cm.insert(entity_id, RefCell::new(component));
                self.component_maps.insert(cm);
            }
        }
    }

    pub fn remove_component<C>(&mut self, entity_id: EntityId)
    where
        C: Component + 'static,
    {
        self.component_maps.get_mut::<ComponentMap<C>>().map(|cm| cm.remove(entity_id));
    }

    #[allow(dead_code)]
    pub fn add_entity_deferred(&self) -> DeferredEntityId {
        let def_id = self.deferred_entity_ids.borrow_mut().insert(Key::null());
        let f = move |ecs: &mut Ecs| {
            let real_id = ecs.add_entity();
            *ecs.deferred_entity_ids.borrow_mut().get_mut(def_id).unwrap() = real_id;
        };
        self.deferred_mutations.borrow_mut().push(Box::new(f));
        def_id
    }

    #[allow(dead_code)]
    pub fn remove_entity_deferred(&self, entity_id: EntityId) {
        self.deferred_mutations.borrow_mut().push(Box::new(move |ecs: &mut Ecs| {
            ecs.remove_entity(entity_id);
        }));
    }

    #[allow(dead_code)]
    pub fn add_component_deferred<E, C>(&self, entity_id: E, component: C)
    where
        E: Into<RealOrDeferredEntityId>,
        C: Component + 'static,
    {
        match entity_id.into() {
            RealOrDeferredEntityId::Real(real_id) => {
                self.deferred_mutations.borrow_mut().push(Box::new(move |ecs: &mut Ecs| {
                    ecs.add_component(real_id, component);
                }));
            }
            RealOrDeferredEntityId::Deferred(def_id) => {
                let f = move |ecs: &mut Ecs| {
                    let real_id = ecs.deferred_entity_ids.borrow().get(def_id).copied();
                    if let Some(real_id) = real_id {
                        ecs.add_component(real_id, component);
                    }
                };
                self.deferred_mutations.borrow_mut().push(Box::new(f));
            }
        };
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
        self.deferred_entity_ids.borrow_mut().clear();
    }
}

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
        component_maps.get::<ComponentMap<C>>().map(|cm| cm.contains_key(id)).unwrap_or(false)
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
        component_maps.get::<ComponentMap<C>>().map(|cm| cm.contains_key(id)).unwrap_or(false)
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
        component_maps.get::<ComponentMap<C>>().map(|cm| cm.contains_key(id)).unwrap_or(false)
    }

    fn borrow(_: EntityId, _: &AnyMap) -> Self::Result<'_> {
        ()
    }
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
        !component_maps.get::<ComponentMap<C>>().map(|cm| cm.contains_key(id)).unwrap_or(false)
    }

    fn borrow(_: EntityId, _: &AnyMap) -> Self::Result<'_> {
        ()
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
