use crate::ecs::{Component, Ecs, EntityId};
use crate::misc::StoryVars;
use crate::{components, loader};
use egui::{Context, Grid, ScrollArea, TextEdit, Ui, Window};
use egui_sdl2_event::EguiSDL2State;
use itertools::Itertools;
use sdl2::video::Window as SdlWindow;
use serde::Serialize;
use std::collections::HashMap;
use std::time::Instant;
use tap::TapFallible;

pub struct DevUi<'window> {
    pub ctx: Context,
    pub state: EguiSDL2State,
    pub window: &'window SdlWindow,
    pub active: bool,
    // Stored intermediately between processing and rendering for convenience
    pub full_output: Option<egui::FullOutput>,
    //
    pub entities_list_window: EntitiesListWindow,
    pub entity_windows: HashMap<EntityId, EntityWindow>,
    pub story_vars_window: StoryVarsWindow,
}

impl<'window> DevUi<'window> {
    pub fn new(window: &'window SdlWindow) -> Self {
        let ctx = Context::default();
        // (state dpi scaling must be initally set to 1 to set the initial screen_rect correctly)
        let state = EguiSDL2State::new(window.size().0, window.size().1, 1.);

        // TODO transparent windows

        Self {
            ctx,
            state,
            window,
            active: false,
            full_output: None,
            entities_list_window: EntitiesListWindow::new(),
            entity_windows: HashMap::new(),
            story_vars_window: StoryVarsWindow::new(),
        }
    }
}

impl DevUi<'_> {
    // Process egui, process output, and save intermediate full_output for rendering later
    pub fn run(
        &mut self,
        start_time: &Instant,
        frame_duration: f32,
        ecs: &mut Ecs,
        story_vars: &mut StoryVars,
    ) {
        if !self.active {
            return;
        }

        let DevUi { state, ctx, window, .. } = self;

        state.update_time(Some(start_time.elapsed().as_secs_f64()), 1. / 60.);
        ctx.begin_pass(state.raw_input.take());

        // Create entity windows for new entities, and delete for deleted entities
        for id in ecs.entity_ids.keys() {
            if !self.entity_windows.contains_key(&id) {
                self.entity_windows.insert(id, EntityWindow::new(id, &ecs));
            };
        }
        self.entity_windows.retain(|&k, _| ecs.entity_ids.contains_key(k));

        // Main dev ui window
        Window::new("Dev UI")
            .title_bar(false)
            .pivot(egui::Align2::RIGHT_TOP)
            .default_pos(ctx.screen_rect().shrink(16.).right_top())
            .default_width(150.)
            .show(&ctx, |ui| {
                ui.label(format!("Frame Duration: {frame_duration:.2}%"));

                ui.toggle_value(&mut self.entities_list_window.open, "Entities");
                ui.toggle_value(&mut self.story_vars_window.open, "Story Vars");

                ui.allocate_space([ui.available_width(), 0.].into());
            });

        // TODO windows check their open status inside their show

        // Entities list window
        if self.entities_list_window.open {
            self.entities_list_window.show(ctx, &mut self.entity_windows);
        }

        // Entity windows
        for window in self.entity_windows.values_mut().filter(|w| w.open) {
            window.show(ctx, ecs);
        }

        // Story vars window
        if self.story_vars_window.open {
            self.story_vars_window.show(ctx, story_vars);
        }

        let full_output = ctx.end_pass();
        // (Looks like this just updates the cursor and the clipboard text)
        state.process_output(window, &full_output.platform_output);
        self.full_output = Some(full_output);
    }
}

pub struct EntitiesListWindow {
    pub open: bool,
    pub filter_string: String,
}

impl EntitiesListWindow {
    pub fn new() -> Self {
        Self { open: false, filter_string: String::new() }
    }

    pub fn show(&mut self, ctx: &Context, entity_windows: &mut HashMap<EntityId, EntityWindow>) {
        Window::new("Entities").default_width(250.).open(&mut self.open).show(&ctx, |ui| {
            ui.add(TextEdit::singleline(&mut self.filter_string).hint_text("Filter"));

            // TODO filter with special terms such as "has:{Component}"
            // filter with multiple space-separated terms

            ScrollArea::vertical().show(ui, |ui| {
                for window in entity_windows
                    .values_mut()
                    .filter(|w| {
                        w.name.as_ref().map(|n| n.contains(&self.filter_string)).unwrap_or(false)
                            || serde_json::to_string(&w.entity_id)
                                .expect("")
                                .contains(&self.filter_string)
                    })
                    .sorted_by_key(|w| w.entity_id)
                {
                    let title = window.title();
                    ui.toggle_value(&mut window.open, title);
                }

                ui.allocate_space([ui.available_width(), 0.].into());
            });
        });
    }
}

pub struct EntityWindow {
    pub entity_id: EntityId,
    pub name: Option<String>,
    pub open: bool,
    // TODO vec box dyn?
    pub position: ComponentCollapsible<components::Position>,
    pub collision: ComponentCollapsible<components::Collision>,
    pub velocity: ComponentCollapsible<components::Velocity>,
    pub facing: ComponentCollapsible<components::Facing>,
    pub camera: ComponentCollapsible<components::Camera>,
    pub sprite: ComponentCollapsible<components::SpriteComp>,
    pub scripts: ComponentCollapsible<components::Scripts>,
    pub animation: ComponentCollapsible<components::AnimationComp>,
    pub char_anims: ComponentCollapsible<components::CharacterAnims>,
    pub dual_anims: ComponentCollapsible<components::DualStateAnims>,
    pub named_anims: ComponentCollapsible<components::NamedAnims>,
    pub walking: ComponentCollapsible<components::Walking>,
    pub interaction: ComponentCollapsible<components::Interaction>,
    pub sfx: ComponentCollapsible<components::SfxEmitter>,
    // (Does not include SineOffsetAnimation)
}

impl EntityWindow {
    pub fn new(entity_id: EntityId, ecs: &Ecs) -> Self {
        // Name is expected to be immutable, so we only have to set it once
        let name = ecs.query_one_with_id::<&components::Name>(entity_id).map(|n| n.0.clone());

        Self {
            entity_id,
            name,
            open: false,
            position: ComponentCollapsible::new(entity_id),
            collision: ComponentCollapsible::new(entity_id),
            velocity: ComponentCollapsible::new(entity_id),
            facing: ComponentCollapsible::new(entity_id),
            camera: ComponentCollapsible::new(entity_id),
            sprite: ComponentCollapsible::new(entity_id),
            scripts: ComponentCollapsible::new(entity_id),
            animation: ComponentCollapsible::new(entity_id),
            char_anims: ComponentCollapsible::new(entity_id),
            dual_anims: ComponentCollapsible::new(entity_id),
            named_anims: ComponentCollapsible::new(entity_id),
            walking: ComponentCollapsible::new(entity_id),
            interaction: ComponentCollapsible::new(entity_id),
            sfx: ComponentCollapsible::new(entity_id),
        }
    }

    pub fn show(&mut self, ctx: &Context, ecs: &mut Ecs) {
        Window::new(self.title()).default_width(300.).open(&mut self.open).show(&ctx, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                if self.name.is_some() {
                    ui.label(serde_json::to_string(&self.entity_id).expect(""));
                }

                self.position.show(ui, ecs);
                self.collision.show(ui, ecs);
                self.velocity.show(ui, ecs);
                self.facing.show(ui, ecs);
                self.camera.show(ui, ecs);
                self.sprite.show(ui, ecs);
                self.scripts.show(ui, ecs);
                self.animation.show(ui, ecs);
                self.char_anims.show(ui, ecs);
                self.dual_anims.show(ui, ecs);
                self.named_anims.show(ui, ecs);
                self.walking.show(ui, ecs);
                self.interaction.show(ui, ecs);
                self.sfx.show(ui, ecs);
            });
        });
    }

    pub fn title(&self) -> String {
        match &self.name {
            Some(name) => name.clone(),
            None => serde_json::to_string(&self.entity_id).expect(""),
        }
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

    pub fn show(&mut self, ui: &mut Ui, ecs: &mut Ecs) {
        let component = ecs.query_one_with_id::<&C>(self.entity_id);
        if component.is_none() {
            self.text.clear();
            self.is_being_edited = false;
            return;
        }

        if !self.is_being_edited {
            self.text = component
                .as_deref()
                .map(|c| serde_json::to_string_pretty(c).expect(""))
                .expect("");
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
                    };

                    if ui.button("Save").clicked() {
                        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&self.text)
                            .tap_err(|e| log::error!("Invalid component JSON (err: \"{e}\")"))
                        {
                            loader::load_single_component_from_value(
                                ecs,
                                self.entity_id,
                                C::name(),
                                &v,
                            );
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

// StoryVarsWindow is implemented with a different philosophy to EntityWindow.
// In EntityWindow, the window stores an instance of ComponentCollapsible for each component type,
// and each ComponentCollapsible stores and manages its own is_being_edited and edit_text.
// In StoryVarsWindow, the window stores a single Option<{key}> referencing the story var being
// edited and the edit_text of that one story var. It does not store a struct for each story var.
// Is one generally better than the other? Is the choice conditional?

pub struct StoryVarsWindow {
    pub open: bool,
    pub filter_string: String,
    pub var_being_edited: Option<String>,
    pub edit_text: String,
}

impl StoryVarsWindow {
    pub fn new() -> Self {
        Self {
            open: false,
            filter_string: String::new(),
            var_being_edited: None,
            edit_text: String::new(),
        }
    }

    pub fn show(&mut self, ctx: &Context, story_vars: &mut StoryVars) {
        Window::new("Story Vars").default_width(250.).open(&mut self.open).show(&ctx, |ui| {
            ui.add(TextEdit::singleline(&mut self.filter_string).hint_text("Filter"));

            ScrollArea::vertical().show(ui, |ui| {
                Grid::new("grid").show(ui, |ui| {
                    for (key, val) in story_vars
                        .0
                        .iter_mut()
                        .filter(|(k, _)| k.contains(&self.filter_string))
                        .sorted()
                    {
                        let is_being_edited =
                            self.var_being_edited.as_ref().is_some_and(|k| k == key);

                        ui.label(key);

                        ui.horizontal(|ui| {
                            let mut val_as_string = val.to_string();
                            let text_ref = if is_being_edited {
                                &mut self.edit_text
                            } else {
                                &mut val_as_string
                            };
                            ui.add_enabled(
                                is_being_edited,
                                TextEdit::singleline(text_ref).desired_width(10.),
                            );

                            if is_being_edited {
                                if ui.button("Cancel").clicked() {
                                    self.var_being_edited = None;
                                    self.edit_text.clear();
                                }
                                if ui.button("Save").clicked() {
                                    if let Ok(i32) = self.edit_text.parse::<i32>() {
                                        *val = i32;
                                    }
                                    self.var_being_edited = None;
                                    self.edit_text.clear();
                                }
                            } else {
                                if ui.button("Edit").clicked() {
                                    self.var_being_edited = Some(key.clone());
                                    self.edit_text = val_as_string.to_string();
                                }
                            }
                        });

                        ui.end_row();
                    }
                });

                ui.allocate_space([ui.available_width(), 0.].into());
            });
        });
    }
}
