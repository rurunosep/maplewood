use super::query::Query;
use crate::components::Name;
use anymap::AnyMap;
use slotmap::{Key, SecondaryMap, SlotMap, new_key_type};
use std::cell::RefCell;

// TODO entity identifier enum that can be String (name) or EntityId
// String and EntityId implement Into<{Identifier}>
// query_one can take Into<{Identifier}>
// camera target can be an {Identifier}, etc

pub trait Component {
    // Unique name of the component
    // By default, it's the unqualified type name
    fn name() -> &'static str {
        std::any::type_name::<Self>().split("::").last().unwrap()
    }
}

new_key_type! { pub struct EntityId; }
new_key_type! { pub struct DeferredEntityId; }

pub enum RealOrDeferredEntityId {
    Real(EntityId),
    Deferred(DeferredEntityId),
}

impl From<EntityId> for RealOrDeferredEntityId {
    fn from(id: EntityId) -> Self {
        Self::Real(id)
    }
}

impl From<DeferredEntityId> for RealOrDeferredEntityId {
    fn from(id: DeferredEntityId) -> Self {
        Self::Deferred(id)
    }
}

type QueryResultIter<'r, Q> = Box<dyn Iterator<Item = <Q as Query>::Result<'r>> + 'r>;
pub type ComponentMap<C> = SecondaryMap<EntityId, RefCell<C>>;

pub struct Ecs {
    // TODO implement slotmap myself so that I can control its serde functionality (and more)
    pub entity_ids: SlotMap<EntityId, ()>,
    pub component_maps: AnyMap,
    #[allow(clippy::type_complexity)]
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

    fn filter<'ecs, Q>(&'ecs self) -> Box<dyn Iterator<Item = EntityId> + 'ecs>
    where
        Q: Query,
    {
        Box::new(self.entity_ids.keys().filter(|id| Q::filter(*id, &self.component_maps)))
    }

    pub fn query<'ecs, Q>(&'ecs self) -> QueryResultIter<'ecs, Q>
    where
        Q: Query,
    {
        Box::new(self.filter::<Q>().map(|id| Q::borrow(id, &self.component_maps)))
    }

    pub fn query_except<'ecs, Q>(&'ecs self, except: EntityId) -> QueryResultIter<'ecs, Q>
    where
        Q: Query,
    {
        Box::new(
            self.filter::<Q>()
                .filter(move |id| *id != except)
                .map(|id| Q::borrow(id, &self.component_maps)),
        )
    }

    // DOES filter in a way that avoids double borrow in a nested query
    // (Because it filters by id first, then runs the query)
    pub fn query_one_with_id<'ecs, Q>(&'ecs self, id: EntityId) -> Option<Q::Result<'ecs>>
    where
        Q: Query,
    {
        Some(id)
            .filter(|id| Q::filter(*id, &self.component_maps))
            .map(|id| Q::borrow(id, &self.component_maps))
    }

    // Does NOT filter in a way that avoids double borrow in a nested query
    // (Because it filters by name during the query)
    // TODO query_one_with_name filters by name before querying
    // Or just leave it since I'm reworking the ecs later anyway?
    pub fn query_one_with_name<'ecs, Q>(&'ecs self, name: &str) -> Option<Q::Result<'ecs>>
    where
        Q: Query + 'static,
    {
        self.query::<(&Name, Q)>().find(|(n, _)| n.as_str() == name).map(|(_, q)| q)
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
        if let Some(cm) = self.component_maps.get_mut::<ComponentMap<C>>() {
            cm.insert(entity_id, RefCell::new(component));
        } else {
            let mut cm = SecondaryMap::<EntityId, RefCell<C>>::new();
            cm.insert(entity_id, RefCell::new(component));
            self.component_maps.insert(cm);
        }
    }

    pub fn remove_component<C>(&mut self, entity_id: EntityId)
    where
        C: Component + 'static,
    {
        self.component_maps.get_mut::<ComponentMap<C>>().map(|cm| cm.remove(entity_id));
    }

    // TODO explain all of this deferred operations code, cause it's confusing af

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
