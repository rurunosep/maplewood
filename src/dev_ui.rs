use crate::components::{Collision, Facing, Position};
use crate::ecs::{Component, Ecs, EntityId};
use crate::loader::load_single_component_from_value;
use sdl2::video::Window;
use serde::Serialize;
use std::time::Instant;

pub struct DevUiData<'window> {
    pub ctx: egui::Context,
    pub state: egui_sdl2_event::EguiSDL2State,
    pub window: &'window Window,
    pub active: bool,
    // Stored intermediately between processing and rendering for convenience
    pub full_output: Option<egui::FullOutput>,
    //
    pub player_components_window: Option<PlayerComponentsWindow>,
}

// Show egui, process output and app state updates (nothing for now), and save intermediate
// full_output for rendering later
pub fn run_dev_ui(
    dev_ui_data: &mut DevUiData<'_>,
    start_time: &Instant,
    //
    frame_duration: f32,
    ecs: &mut Ecs,
) {
    if !dev_ui_data.active {
        return;
    }

    let DevUiData { state, ctx, window, .. } = dev_ui_data;

    state.update_time(Some(start_time.elapsed().as_secs_f64()), 1. / 60.);
    ctx.begin_pass(state.raw_input.take());

    let mut player_components_window_open = dev_ui_data.player_components_window.is_some();

    egui::Window::new("Debug")
        .pivot(egui::Align2::RIGHT_TOP)
        .default_pos(ctx.screen_rect().shrink(16.).right_top())
        .default_width(150.)
        .show(&ctx, |ui| {
            ui.label(format!("Frame Duration: {frame_duration:.2}%"));
            ui.toggle_value(&mut player_components_window_open, "Player Components");
            ui.allocate_space([ui.available_width(), 0.].into());
        });

    if let Some(window) = &mut dev_ui_data.player_components_window {
        window.show(ctx, &mut player_components_window_open, ecs);
    }
    match (player_components_window_open, &dev_ui_data.player_components_window) {
        (true, None) => {
            dev_ui_data.player_components_window = Some(PlayerComponentsWindow::new())
        }
        (false, Some(_)) => dev_ui_data.player_components_window = None,
        _ => {}
    };

    let full_output = ctx.end_pass();
    // (Looks like this just updates the cursor and the clipboard text)
    state.process_output(window, &full_output.platform_output);
    dev_ui_data.full_output = Some(full_output);
}

pub struct PlayerComponentsWindow {
    pub position_collapsible: Option<ComponentCollapsible<Position>>,
    pub collision_collapsible: Option<ComponentCollapsible<Collision>>,
    pub facing_collapsible: Option<ComponentCollapsible<Facing>>,
}

impl PlayerComponentsWindow {
    pub fn new() -> Self {
        Self { position_collapsible: None, collision_collapsible: None, facing_collapsible: None }
    }

    pub fn show(&mut self, ctx: &egui::Context, open: &mut bool, ecs: &mut Ecs) {
        egui::Window::new("Player Components").default_width(250.).open(open).show(&ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                let player_id = ecs.query_one_with_name::<EntityId>("player").unwrap();

                // These are pretty verbose
                match (ecs.query_one_with_id::<&Position>(player_id), &self.position_collapsible)
                {
                    (Some(_), None) => {
                        self.position_collapsible =
                            Some(ComponentCollapsible::<Position>::new(player_id, "position"));
                    }
                    (None, Some(_)) => {
                        self.position_collapsible = None;
                    }
                    _ => {}
                };

                match (
                    ecs.query_one_with_id::<&Collision>(player_id),
                    &self.collision_collapsible,
                ) {
                    (Some(_), None) => {
                        self.collision_collapsible =
                            Some(ComponentCollapsible::<Collision>::new(player_id, "collision"));
                    }
                    (None, Some(_)) => {
                        self.collision_collapsible = None;
                    }
                    _ => {}
                };

                match (ecs.query_one_with_id::<&Facing>(player_id), &self.facing_collapsible) {
                    (Some(_), None) => {
                        self.facing_collapsible =
                            Some(ComponentCollapsible::<Facing>::new(player_id, "facing"));
                    }
                    (None, Some(_)) => {
                        self.facing_collapsible = None;
                    }
                    _ => {}
                };

                if let Some(c) = &mut self.position_collapsible {
                    c.show(ui, ecs);
                }
                if let Some(c) = &mut self.collision_collapsible {
                    c.show(ui, ecs);
                }
                if let Some(c) = &mut self.facing_collapsible {
                    c.show(ui, ecs);
                }
            });
        });
    }
}

pub struct ComponentCollapsible<C> {
    pub entity_id: EntityId,
    pub name: &'static str,
    pub text: String,
    pub editable: bool,
    pub _component: std::marker::PhantomData<C>,
}

impl<C> ComponentCollapsible<C>
where
    C: Component + Serialize + 'static,
{
    pub fn new(entity_id: EntityId, name: &'static str) -> Self {
        Self {
            entity_id,
            name,
            text: String::new(),
            editable: false,
            _component: std::marker::PhantomData::<C>,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, ecs: &mut Ecs) {
        ui.collapsing(self.name, |ui| {
            if !self.editable {
                self.text = serde_json::to_string_pretty(
                    &*ecs.query_one_with_id::<&C>(self.entity_id).unwrap(),
                )
                .unwrap();
            }

            ui.add_enabled(
                self.editable,
                egui::TextEdit::multiline(&mut self.text).code_editor().desired_rows(1),
            );

            ui.horizontal(|ui| {
                if self.editable {
                    if ui.button("Cancel").clicked() {
                        self.editable = false;
                    };

                    if ui.button("Save").clicked() {
                        load_single_component_from_value(
                            ecs,
                            self.entity_id,
                            self.name,
                            &serde_json::from_str::<serde_json::Value>(&self.text).unwrap(),
                        );

                        self.editable = false;
                    };
                } else {
                    if ui.button("Edit").clicked() {
                        self.editable = true;
                    };
                }
            });
        });
    }
}
