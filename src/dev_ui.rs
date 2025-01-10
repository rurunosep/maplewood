use crate::components::{Camera, Collision, Facing, Name, Position};
use crate::ecs::{Component, Ecs, EntityId};
use crate::loader::load_single_component_from_value;
use itertools::Itertools;
use sdl2::video::Window;
use serde::Serialize;
use std::collections::HashMap;
use std::time::Instant;
use tap::TapFallible;

pub struct DevUi<'window> {
    pub ctx: egui::Context,
    pub state: egui_sdl2_event::EguiSDL2State,
    pub window: &'window Window,
    pub active: bool,
    // Stored intermediately between processing and rendering for convenience
    pub full_output: Option<egui::FullOutput>,
    //
    pub entities_list_window: EntitiesListWindow,
    pub entity_windows: HashMap<EntityId, EntityWindow>,
}

impl<'window> DevUi<'window> {
    pub fn new(window: &'window Window) -> Self {
        let ctx = egui::Context::default();
        // (state dpi scaling must be initally set to 1 to set the initial screen_rect correctly)
        let state = egui_sdl2_event::EguiSDL2State::new(window.size().0, window.size().1, 1.);

        Self {
            ctx,
            state,
            window,
            active: false,
            full_output: None,
            entities_list_window: EntitiesListWindow::new(),
            entity_windows: HashMap::new(),
        }
    }
}

impl DevUi<'_> {
    // Process egui, process output, and save intermediate full_output for rendering later
    pub fn run(&mut self, start_time: &Instant, frame_duration: f32, ecs: &mut Ecs) {
        if !self.active {
            return;
        }

        let DevUi { state, ctx, window, .. } = self;

        state.update_time(Some(start_time.elapsed().as_secs_f64()), 1. / 60.);
        ctx.begin_pass(state.raw_input.take());

        // Create entity windows for new entities
        for id in ecs.entity_ids.keys() {
            if !self.entity_windows.contains_key(&id) {
                self.entity_windows.insert(id, EntityWindow::new(id));
            };
        }
        // NOW remove windows for entities that don't exist anymore

        // Main debug window
        egui::Window::new("Debug")
            .pivot(egui::Align2::RIGHT_TOP)
            .default_pos(ctx.screen_rect().shrink(16.).right_top())
            .default_width(150.)
            .show(&ctx, |ui| {
                ui.label(format!("Frame Duration: {frame_duration:.2}%"));

                ui.toggle_value(&mut self.entities_list_window.open, "Entities");

                ui.allocate_space([ui.available_width(), 0.].into());
            });

        // Entities list window
        if self.entities_list_window.open {
            self.entities_list_window.show(ctx, &mut self.entity_windows, ecs);
        }

        // Entity windows
        for window in self.entity_windows.values_mut().filter(|w| w.open) {
            window.show(ctx, ecs);
        }

        let full_output = ctx.end_pass();
        // (Looks like this just updates the cursor and the clipboard text)
        state.process_output(window, &full_output.platform_output);
        self.full_output = Some(full_output);
    }
}

pub struct EntitiesListWindow {
    pub open: bool,
}

impl EntitiesListWindow {
    pub fn new() -> Self {
        Self { open: false }
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        entity_windows: &mut HashMap<EntityId, EntityWindow>,
        ecs: &Ecs,
    ) {
        egui::Window::new("Entities").open(&mut self.open).show(&ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                for window in entity_windows.values_mut().sorted_by_key(|w| w.entity_id) {
                    let title = ecs
                        .query_one_with_id::<&Name>(window.entity_id)
                        .map(|n| n.0.clone())
                        .unwrap_or(serde_json::to_string(&window.entity_id).expect(""));

                    ui.toggle_value(&mut window.open, title);
                }
            });
        });
    }
}

pub struct EntityWindow {
    pub entity_id: EntityId,
    pub open: bool,
    pub position_collapsible: Option<ComponentCollapsible<Position>>,
    pub collision_collapsible: Option<ComponentCollapsible<Collision>>,
    pub facing_collapsible: Option<ComponentCollapsible<Facing>>,
    pub camera_collapsible: Option<ComponentCollapsible<Camera>>,
}

impl EntityWindow {
    pub fn new(entity_id: EntityId) -> Self {
        Self {
            entity_id,
            open: false,
            position_collapsible: None,
            collision_collapsible: None,
            facing_collapsible: None,
            camera_collapsible: None,
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, ecs: &mut Ecs) {
        // NOW: compress this
        match (ecs.query_one_with_id::<&Position>(self.entity_id), &self.position_collapsible) {
            (Some(_), None) => {
                self.position_collapsible =
                    Some(ComponentCollapsible::<Position>::new(self.entity_id));
            }
            (None, Some(_)) => {
                self.position_collapsible = None;
            }
            _ => {}
        };

        match (ecs.query_one_with_id::<&Collision>(self.entity_id), &self.collision_collapsible) {
            (Some(_), None) => {
                self.collision_collapsible =
                    Some(ComponentCollapsible::<Collision>::new(self.entity_id));
            }
            (None, Some(_)) => {
                self.collision_collapsible = None;
            }
            _ => {}
        };

        match (ecs.query_one_with_id::<&Facing>(self.entity_id), &self.facing_collapsible) {
            (Some(_), None) => {
                self.facing_collapsible =
                    Some(ComponentCollapsible::<Facing>::new(self.entity_id));
            }
            (None, Some(_)) => {
                self.facing_collapsible = None;
            }
            _ => {}
        };

        match (ecs.query_one_with_id::<&Camera>(self.entity_id), &self.camera_collapsible) {
            (Some(_), None) => {
                self.camera_collapsible =
                    Some(ComponentCollapsible::<Camera>::new(self.entity_id));
            }
            (None, Some(_)) => {
                self.facing_collapsible = None;
            }
            _ => {}
        };

        let name = ecs.query_one_with_id::<&Name>(self.entity_id).map(|n| n.0.clone());
        let has_name = name.is_some();
        let title = name.unwrap_or(serde_json::to_string(&self.entity_id).expect(""));

        egui::Window::new(title).default_width(250.).open(&mut self.open).show(&ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                if has_name {
                    ui.label(serde_json::to_string(&self.entity_id).expect(""));
                }

                if let Some(c) = &mut self.position_collapsible {
                    c.show(ui, ecs);
                }
                if let Some(c) = &mut self.collision_collapsible {
                    c.show(ui, ecs);
                }
                if let Some(c) = &mut self.facing_collapsible {
                    c.show(ui, ecs);
                }
                if let Some(c) = &mut self.camera_collapsible {
                    c.show(ui, ecs);
                }
            });
        });
    }
}

pub struct ComponentCollapsible<C> {
    pub entity_id: EntityId,
    pub text: String,
    pub is_being_edited: bool,
    pub _component: std::marker::PhantomData<C>,
}

impl<C> ComponentCollapsible<C>
where
    C: Component + Serialize + 'static,
{
    pub fn new(entity_id: EntityId) -> Self {
        Self {
            entity_id,
            text: String::new(),
            is_being_edited: false,
            _component: std::marker::PhantomData::<C>,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, ecs: &mut Ecs) {
        ui.collapsing(C::name(), |ui| {
            if !self.is_being_edited {
                self.text = ecs
                    .query_one_with_id::<&C>(self.entity_id)
                    .map(|c| serde_json::to_string_pretty(&*c).expect(""))
                    .unwrap_or_default();
            }

            ui.add_enabled(
                self.is_being_edited,
                egui::TextEdit::multiline(&mut self.text).code_editor().desired_rows(1),
            );

            ui.horizontal(|ui| {
                if self.is_being_edited {
                    if ui.button("Cancel").clicked() {
                        self.is_being_edited = false;
                    };

                    if ui.button("Save").clicked() {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&self.text)
                            .tap_err(|e| log::error!("Invalid component JSON (err: \"{e}\")"))
                        {
                            load_single_component_from_value(ecs, self.entity_id, C::name(), &v);
                        }

                        self.is_being_edited = false;
                    };
                } else {
                    if ui.button("Edit").clicked() {
                        self.is_being_edited = true;
                    };
                }
            });
        });
    }
}

pub struct ImmutableComponentCollapsible<C> {
    pub entity_id: EntityId,
    pub _component: std::marker::PhantomData<C>,
}

#[allow(unused)]
impl<C> ImmutableComponentCollapsible<C>
where
    C: Component + Serialize + 'static,
{
    pub fn new(entity_id: EntityId) -> Self {
        Self { entity_id, _component: std::marker::PhantomData::<C> }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, ecs: &mut Ecs) {
        ui.collapsing(C::name(), |ui| {
            ui.add_enabled(
                false,
                egui::TextEdit::multiline(
                    &mut ecs
                        .query_one_with_id::<&C>(self.entity_id)
                        .map(|c| serde_json::to_string_pretty(&*c).expect(""))
                        .unwrap_or_default(),
                )
                .code_editor()
                .desired_rows(1),
            );
        });
    }
}
