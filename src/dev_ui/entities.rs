use crate::components::*;
use crate::ecs::{Component, Ecs, EntityId};
use egui::{Context, ScrollArea, TextEdit, Ui, Window};
use itertools::Itertools;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::format as f;
use tap::TapFallible;

pub struct EntitiesListWindow {
    pub open: bool,
    filter_string: String,
}

impl EntitiesListWindow {
    pub fn new() -> Self {
        Self { open: false, filter_string: String::new() }
    }

    pub fn show(&mut self, ctx: &Context, entity_windows: &mut HashMap<EntityId, EntityWindow>) {
        if !self.open {
            return;
        }

        Window::new("Entities").open(&mut self.open).default_width(250.).show(ctx, |ui| {
            ui.add(TextEdit::singleline(&mut self.filter_string).hint_text("Filter"));

            // TODO filter with special terms such as "has:{Component}"
            // TODO filter with multiple space-separated terms

            ScrollArea::vertical().show(ui, |ui| {
                for window in entity_windows
                    .values_mut()
                    .filter(|window| {
                        window.name.as_ref().is_some_and(|name| name.contains(&self.filter_string))
                            || serde_json::to_string(&window.entity_id)
                                .expect("id is serde")
                                .contains(&self.filter_string)
                    })
                    .sorted_by_key(|window| window.entity_id)
                {
                    let title = window.name();
                    ui.toggle_value(&mut window.open, title);
                }

                // (pad if below window.default_width)
                ui.allocate_space([ui.available_width(), 0.].into());
            });
        });
    }
}

pub struct EntityWindow {
    pub open: bool,
    window_id: egui::Id,
    entity_id: EntityId,
    name: Option<String>,
    component_collapsibles: Vec<Box<dyn CcTrait>>,
}

impl EntityWindow {
    pub fn new(entity_id: EntityId, ecs: &Ecs) -> Self {
        // Name is expected to be immutable, so we only have to set it once
        let name = ecs.query_one_with_id::<&Name>(entity_id).map(|n| n.0.clone());

        let window_id = egui::Id::new(f!("entity {entity_id:?}"));

        let ccs: Vec<Box<dyn CcTrait>> = vec![
            Box::new(ComponentCollapsible::<Position>::new(entity_id)),
            Box::new(ComponentCollapsible::<Collision>::new(entity_id)),
            Box::new(ComponentCollapsible::<Velocity>::new(entity_id)),
            Box::new(ComponentCollapsible::<Facing>::new(entity_id)),
            Box::new(ComponentCollapsible::<Camera>::new(entity_id)),
            Box::new(ComponentCollapsible::<SpriteComp>::new(entity_id)),
            Box::new(ComponentCollapsible::<AnimationComp>::new(entity_id)),
            Box::new(ComponentCollapsible::<CharacterAnims>::new(entity_id)),
            Box::new(ComponentCollapsible::<DualStateAnims>::new(entity_id)),
            Box::new(ComponentCollapsible::<NamedAnims>::new(entity_id)),
            Box::new(ComponentCollapsible::<Walking>::new(entity_id)),
            Box::new(ComponentCollapsible::<SfxEmitter>::new(entity_id)),
            Box::new(ComponentCollapsible::<InteractionTrigger>::new(entity_id)),
            Box::new(ComponentCollapsible::<CollisionTrigger>::new(entity_id)),
            Box::new(ComponentCollapsible::<AreaTrigger>::new(entity_id)),
        ];

        Self { open: false, window_id, entity_id, name, component_collapsibles: ccs }
    }

    pub fn show(&mut self, ctx: &Context, ecs: &mut Ecs) {
        if !self.open {
            return;
        }

        Window::new(f!("Entity: {}", self.name()))
            .id(self.window_id)
            .open(&mut self.open)
            .default_width(300.)
            .show(ctx, |ui| {
                ScrollArea::vertical().show(ui, |ui| {
                    if self.name.is_some() {
                        ui.label(serde_json::to_string(&self.entity_id).expect(""));
                    }

                    for cc in &mut self.component_collapsibles {
                        cc.show(ui, ecs);
                    }
                });
            });
    }

    pub fn name(&self) -> String {
        match &self.name {
            Some(name) => name.clone(),
            None => serde_json::to_string(&self.entity_id).expect(""),
        }
    }
}

struct ComponentCollapsible<C> {
    entity_id: EntityId,
    text: String,
    is_being_edited: bool,
    _component: std::marker::PhantomData<C>,
}

impl<C> ComponentCollapsible<C>
where
    C: Component + Serialize + 'static,
{
    fn new(entity_id: EntityId) -> Self {
        Self {
            entity_id,
            text: String::new(),
            is_being_edited: false,
            _component: std::marker::PhantomData::<C>,
        }
    }
}

// I'd like to rename tne generic struct and the trait, but idk what to call them
trait CcTrait {
    fn show(&mut self, ui: &mut Ui, ecs: &mut Ecs);
}

impl<C> CcTrait for ComponentCollapsible<C>
where
    C: Component + Serialize + DeserializeOwned + 'static,
{
    fn show(&mut self, ui: &mut Ui, ecs: &mut Ecs) {
        let component = ecs.query_one_with_id::<&C>(self.entity_id);
        if component.is_none() {
            self.text.clear();
            self.is_being_edited = false;
            return;
        }

        if !self.is_being_edited {
            self.text =
                component.as_deref().map(|c| serde_json::to_string_pretty(c).expect("")).expect("");
        }

        drop(component);

        ui.collapsing(C::name(), |ui| {
            ui.add(
                TextEdit::multiline(&mut self.text)
                    .code_editor()
                    .desired_rows(1)
                    .interactive(self.is_being_edited),
            );

            ui.horizontal(|ui| {
                if self.is_being_edited {
                    if ui.button("Cancel").clicked() {
                        self.is_being_edited = false;
                    }

                    if ui.button("Save").clicked() {
                        if let Ok(component) = serde_json::from_str::<C>(&self.text)
                            .tap_err(|e| log::error!("Couldn't edit component (err: {e})"))
                        {
                            ecs.add_component(self.entity_id, component);
                        }

                        self.is_being_edited = false;
                    }
                } else {
                    if ui.button("Edit").clicked() {
                        self.is_being_edited = true;
                    }
                }
            });
        });
    }
}
