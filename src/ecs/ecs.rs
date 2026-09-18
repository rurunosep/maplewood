use super::query::Query;
use crate::components::*;
use anyhow::anyhow;
use anymap::AnyMap;
use serde::Serialize;
use serde::de::DeserializeOwned;
use slotmap::{Key, SecondaryMap, SlotMap, new_key_type};
use std::cell::RefCell;
use std::collections::HashMap;
use std::error::Error;
use std::fmt::Display;

pub enum EntityIdentifier {
    Id(EntityId),
    Name(String),
}

impl From<EntityId> for EntityIdentifier {
    fn from(id: EntityId) -> Self {
        Self::Id(id)
    }
}

impl<T: AsRef<str>> From<T> for EntityIdentifier {
    fn from(name: T) -> Self {
        Self::Name(name.as_ref().to_string())
    }
}

pub trait Component {
    // Unique name of the component
    // By default, it's the unqualified type name
    fn name() -> &'static str {
        std::any::type_name::<Self>().split("::").last().expect("split always returns at least 1")
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
// TODO rework ecs without refcells
pub type ComponentMap<C> = SecondaryMap<EntityId, RefCell<C>>;

pub struct Ecs {
    // TODO implement slotmap myself so that I can control its serde functionality
    pub entity_ids: SlotMap<EntityId, ()>,
    component_maps: AnyMap,
    name_to_id: HashMap<String, EntityId>,
    deferred_mutations: RefCell<Vec<Box<dyn FnOnce(&mut Ecs)>>>,
    deferred_entity_ids: RefCell<SlotMap<DeferredEntityId, EntityId>>,
}

impl Ecs {
    pub fn new() -> Self {
        Self {
            entity_ids: SlotMap::with_key(),
            component_maps: AnyMap::new(),
            name_to_id: HashMap::new(),
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

    pub fn query_one<'ecs, Q>(
        &'ecs self,
        identifier: impl Into<EntityIdentifier>,
    ) -> Result<Q::Result<'ecs>, QueryOneError>
    where
        Q: Query,
    {
        let id = match identifier.into() {
            EntityIdentifier::Id(id) => id,
            EntityIdentifier::Name(name) => {
                *self.name_to_id.get(&name).ok_or(QueryOneError::NoEntity)?
            }
        };

        if !self.entity_ids.contains_key(id) {
            return Err(QueryOneError::NoEntity);
        }

        Some(id)
            .filter(|id| Q::filter(*id, &self.component_maps))
            .map(|id| Q::borrow(id, &self.component_maps))
            .ok_or(QueryOneError::MissingComponents)
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
        // If the component is a Name, register it to the name_to_id map
        // (This should be optimized out by the compiler for every other component, probably)
        if std::any::TypeId::of::<C>() == std::any::TypeId::of::<Name>() {
            // SAFETY: we checked that C is in fact Name
            let name = unsafe { &*(&component as *const C as *const Name) };
            self.name_to_id.insert(name.0.clone(), entity_id);
        }

        self.component_maps
            .entry::<ComponentMap<C>>()
            .or_insert_with(|| ComponentMap::new())
            .insert(entity_id, RefCell::new(component));
    }

    pub fn remove_component<C>(&mut self, entity_id: EntityId)
    where
        C: Component + 'static,
    {
        // If the component is a Name, deregister it from the name_to_id map
        // (This should be optimized out by the compiler for every other component, probably)
        if std::any::TypeId::of::<C>() == std::any::TypeId::of::<Name>() {
            // TODO make this constant time with a BiHashMap instead of a HashMap?
            self.name_to_id.retain(|_, v| *v != entity_id);
        }

        self.component_maps.get_mut::<ComponentMap<C>>().map(|cm| cm.remove(entity_id));
    }

    // TODO explain all of this deferred operations code, cause it's confusing af

    #[allow(dead_code)]
    pub fn add_entity_deferred(&self) -> DeferredEntityId {
        let def_id = self.deferred_entity_ids.borrow_mut().insert(Key::null());
        let f = move |ecs: &mut Ecs| {
            let real_id = ecs.add_entity();
            *ecs.deferred_entity_ids
                .borrow_mut()
                .get_mut(def_id)
                .expect("def ids only cleared after all closures executed") = real_id;
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
        }
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

    // TODO single match-on-name func that takes callback?

    pub fn add_component_with_name(
        &mut self,
        id: EntityId,
        component_name: &str,
        data: &serde_json::Value,
    ) -> anyhow::Result<()> {
        let data_c = data.clone();

        // Cause conversion from serde error to anyhow error with ? in the try block isn't working
        fn sjfv<T: DeserializeOwned>(data: serde_json::Value) -> anyhow::Result<T> {
            Ok(serde_json::from_value(data)?)
        }

        let r: anyhow::Result<()> = try {
            match component_name {
                "Name" => self.add_component(id, sjfv::<Name>(data_c)?),
                "Position" => self.add_component(id, sjfv::<Position>(data_c)?),
                "Velocity" => self.add_component(id, sjfv::<Velocity>(data_c)?),
                "Collision" => self.add_component(id, sjfv::<Collision>(data_c)?),
                "SfxEmitter" => self.add_component(id, sjfv::<SfxEmitter>(data_c)?),
                "SpriteComp" => self.add_component(id, sjfv::<SpriteComp>(data_c)?),
                "Facing" => self.add_component(id, sjfv::<Facing>(data_c)?),
                "Walking" => self.add_component(id, sjfv::<Walking>(data_c)?),
                "Pathing" => self.add_component(id, sjfv::<Pathing>(data_c)?),
                "Camera" => self.add_component(id, sjfv::<Camera>(data_c)?),
                "AnimationComp" => self.add_component(id, sjfv::<AnimationComp>(data_c)?),
                "CharacterAnims" => self.add_component(id, sjfv::<CharacterAnims>(data_c)?),
                "DualStateAnims" => self.add_component(id, sjfv::<DualStateAnims>(data_c)?),
                "NamedAnims" => self.add_component(id, sjfv::<NamedAnims>(data_c)?),
                "InteractionTrigger" => self.add_component(id, sjfv::<InteractionTrigger>(data_c)?),
                "CollisionTrigger" => self.add_component(id, sjfv::<CollisionTrigger>(data_c)?),
                "AreaTrigger" => self.add_component(id, sjfv::<AreaTrigger>(data_c)?),
                "OverheadText" => self.add_component(id, sjfv::<OverheadText>(data_c)?),
                _ => Err(anyhow!("invalid component name `{component_name}`"))?,
            }
        };
        r.map_err(|e| {
            anyhow!(
                "invalid json component\nname: {component_name}\ndata: {}\nerr: {e}",
                serde_json::to_string_pretty(&data).expect("is serde")
            )
        })
    }

    pub fn remove_component_with_name(
        &mut self,
        id: EntityId,
        component_name: &str,
    ) -> anyhow::Result<()> {
        match component_name {
            "Name" => self.remove_component::<Name>(id),
            "Position" => self.remove_component::<Position>(id),
            "Velocity" => self.remove_component::<Velocity>(id),
            "Collision" => self.remove_component::<Collision>(id),
            "SfxEmitter" => self.remove_component::<SfxEmitter>(id),
            "SpriteComp" => self.remove_component::<SpriteComp>(id),
            "Facing" => self.remove_component::<Facing>(id),
            "Walking" => self.remove_component::<Walking>(id),
            "Pathing" => self.remove_component::<Pathing>(id),
            "Camera" => self.remove_component::<Camera>(id),
            "AnimationComp" => self.remove_component::<AnimationComp>(id),
            "CharacterAnims" => self.remove_component::<CharacterAnims>(id),
            "DualStateAnims" => self.remove_component::<DualStateAnims>(id),
            "NamedAnims" => self.remove_component::<NamedAnims>(id),
            "InteractionTrigger" => self.remove_component::<InteractionTrigger>(id),
            "CollisionTrigger" => self.remove_component::<CollisionTrigger>(id),
            "AreaTrigger" => self.remove_component::<AreaTrigger>(id),
            "OverheadText" => self.remove_component::<OverheadText>(id),
            _ => return Err(anyhow!("invalid component name `{component_name}`")),
        }

        Ok(())
    }

    // This is only for debug or for easily generating component json
    // It doesn't include an id for restoring game state
    pub fn save_components_to_value(&self, id: EntityId) -> serde_json::Value {
        fn insert<C>(
            components: &mut serde_json::Map<String, serde_json::Value>,
            id: EntityId,
            ecs: &Ecs,
        ) where
            C: Component + Clone + Serialize + 'static,
        {
            if let Ok(component) = ecs.query_one::<&C>(id)
                && let Ok(value) = serde_json::to_value(component.clone())
            {
                components.insert(C::name().to_string(), value);
            }
        }

        let mut components = serde_json::Map::new();

        insert::<Name>(&mut components, id, self);
        insert::<Position>(&mut components, id, self);
        insert::<Velocity>(&mut components, id, self);
        insert::<Collision>(&mut components, id, self);
        insert::<SfxEmitter>(&mut components, id, self);
        insert::<SpriteComp>(&mut components, id, self);
        insert::<Facing>(&mut components, id, self);
        insert::<Walking>(&mut components, id, self);
        insert::<Camera>(&mut components, id, self);
        insert::<AnimationComp>(&mut components, id, self);
        insert::<CharacterAnims>(&mut components, id, self);
        insert::<DualStateAnims>(&mut components, id, self);
        insert::<NamedAnims>(&mut components, id, self);
        insert::<InteractionTrigger>(&mut components, id, self);
        insert::<CollisionTrigger>(&mut components, id, self);
        insert::<AreaTrigger>(&mut components, id, self);

        serde_json::Value::Object(components)
    }
}

#[derive(Debug)]
pub enum QueryOneError {
    // TODO include the identifier?
    NoEntity,
    // TODO include a list of missing component names?
    MissingComponents,
}

impl Error for QueryOneError {}

impl Display for QueryOneError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QueryOneError::NoEntity => write!(f, "queried entity doesn't exist"),
            QueryOneError::MissingComponents => write!(f, "missing queried components"),
        }
    }
}
