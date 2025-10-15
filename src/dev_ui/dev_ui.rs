use crate::Console;
use crate::dev_ui::entities::{EntitiesListWindow, EntityWindow};
use crate::ecs::{Ecs, EntityId};
use crate::misc::StoryVars;
use crate::script::{ScriptInstanceId, ScriptManager};
use egui::text::LayoutJob;
use egui::{
    CentralPanel, Color32, Context, FontFamily, FontId, Grid, Key, Label, RichText, ScrollArea,
    TextEdit, TextFormat, TextStyle, TopBottomPanel, Widget, Window,
};
use egui_sdl2_event::EguiSDL2State;
use itertools::Itertools;
use sdl2::video::Window as SdlWindow;
use std::collections::HashMap;
use std::format as f;
use std::time::Instant;

pub struct DevUi<'window> {
    pub ctx: Context,
    pub state: EguiSDL2State,
    pub window: &'window SdlWindow,
    pub active: bool,
    // Stored intermediately between processing and rendering for convenience
    pub full_output: Option<egui::FullOutput>,
    entities_list_window: EntitiesListWindow,
    entity_windows: HashMap<EntityId, EntityWindow>,
    scripts_list_window: ScriptsListWindow,
    script_windows: HashMap<ScriptInstanceId, ScriptWindow>,
    story_vars_window: StoryVarsWindow,
    console_window: ConsoleWindow,
}

impl<'window> DevUi<'window> {
    pub fn new(window: &'window SdlWindow) -> Self {
        let ctx = Context::default();
        // State dpi scaling must be initally set to 1 to set the initial screen_rect correctly
        let state = EguiSDL2State::new(window.size().0, window.size().1, 1.);

        // Some translucent styling
        ctx.style_mut(|s| {
            s.visuals.window_fill = s.visuals.window_fill.gamma_multiply(0.95);
            s.visuals.panel_fill = s.visuals.panel_fill.gamma_multiply(0.5);
            s.visuals.extreme_bg_color = s.visuals.extreme_bg_color.gamma_multiply(0.5);
        });

        Self {
            ctx,
            state,
            window,
            active: false,
            full_output: None,
            entities_list_window: EntitiesListWindow::new(),
            entity_windows: HashMap::new(),
            scripts_list_window: ScriptsListWindow::new(),
            script_windows: HashMap::new(),
            story_vars_window: StoryVarsWindow::new(),
            console_window: ConsoleWindow::new(),
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
        script_manager: &ScriptManager,
        console: &mut Console,
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

        // ... script windows ...
        for (id, instance) in &script_manager.instances {
            if !self.script_windows.contains_key(&id) {
                self.script_windows.insert(id, ScriptWindow::new(id, instance.name.clone()));
            };
        }
        self.script_windows.retain(|&k, _| script_manager.instances.contains_key(k));

        // Main dev ui window
        Window::new("Dev UI")
            .title_bar(false)
            .pivot(egui::Align2::RIGHT_TOP)
            .default_pos(ctx.screen_rect().shrink(16.).right_top())
            .default_width(150.)
            .show(&ctx, |ui| {
                ui.label(f!("Frame Duration: {frame_duration:.2}%"));

                ui.toggle_value(&mut self.console_window.open, "Console");
                ui.toggle_value(&mut self.entities_list_window.open, "Entities");
                ui.toggle_value(&mut self.story_vars_window.open, "Story Vars");
                ui.toggle_value(&mut self.scripts_list_window.open, "Scripts");

                ui.allocate_space([ui.available_width(), 0.].into());
            });

        // Other windows
        self.console_window.show(ctx, console);
        self.entities_list_window.show(ctx, &mut self.entity_windows);
        for window in self.entity_windows.values_mut() {
            window.show(ctx, ecs);
        }
        self.scripts_list_window.show(ctx, &mut self.script_windows);
        for window in self.script_windows.values_mut() {
            window.show(ctx, script_manager);
        }
        self.story_vars_window.show(ctx, story_vars);

        let full_output = ctx.end_pass();
        // (Looks like this just updates the cursor and the clipboard text)
        state.process_output(window, &full_output.platform_output);
        self.full_output = Some(full_output);
    }
}

pub struct ConsoleWindow {
    pub open: bool,
    input_text: String,
}

impl ConsoleWindow {
    fn new() -> Self {
        Self { open: false, input_text: String::new() }
    }

    fn show(&mut self, ctx: &Context, console: &mut Console) {
        if !self.open {
            return;
        }

        Window::new("Console").open(&mut self.open).default_width(800.).show(&ctx, |ui| {
            // Input
            TopBottomPanel::bottom("bottom").show_inside(ui, |ui| {
                let response = TextEdit::singleline(&mut self.input_text)
                    .desired_width(f32::INFINITY)
                    .font(TextStyle::Monospace)
                    .return_key(None)
                    .frame(false)
                    .ui(ui);

                if response.has_focus() {
                    ui.input(|i| {
                        if i.key_pressed(Key::Enter) {
                            console.push(&f!("> {}", &self.input_text));
                            console.command_queue.push(self.input_text.clone());
                            self.input_text.clear();
                        } else if i.key_pressed(Key::ArrowUp) {
                            // NOW scroll input history
                        } else if i.key_pressed(Key::ArrowDown) {
                            // NOW scroll input history
                        } else if i.key_pressed(Key::Tab) {
                            // TODO tab completion
                        }
                    })
                }
            });

            // Log
            CentralPanel::default().show_inside(ui, |ui| {
                ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
                    Label::new(
                        RichText::new(&*console.output_history).family(FontFamily::Monospace),
                    )
                    .ui(ui);

                    ui.allocate_space([ui.available_width(), 0.].into());
                })
            })
        });
    }
}

struct ScriptsListWindow {
    open: bool,
}

impl ScriptsListWindow {
    fn new() -> Self {
        Self { open: false }
    }

    fn show(
        &mut self,
        ctx: &Context,
        script_windows: &mut HashMap<ScriptInstanceId, ScriptWindow>,
    ) {
        if !self.open {
            return;
        }

        Window::new("Scripts").open(&mut self.open).default_width(250.).show(&ctx, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                for window in script_windows.values_mut() {
                    let name = window.name();
                    ui.toggle_value(&mut window.open, name);
                }

                if script_windows.is_empty() {
                    ui.label("No active scripts");
                }

                // Pad if below window.default_width
                ui.allocate_space([ui.available_width(), 0.].into());
            })
        });
    }
}

struct ScriptWindow {
    open: bool,
    window_id: egui::Id,
    script_id: ScriptInstanceId,
    name: Option<String>,
}

impl ScriptWindow {
    fn new(script_id: ScriptInstanceId, name: Option<String>) -> Self {
        let window_id = egui::Id::new(f!("script {script_id:?}"));
        Self { open: false, window_id, script_id, name }
    }

    fn show(&mut self, ctx: &Context, script_manager: &ScriptManager) {
        if !self.open {
            return;
        }

        let Some(script_instance) = script_manager.instances.get(self.script_id) else {
            return;
        };

        let source = &script_instance.source;

        let Ok(current_line_index) =
            script_instance.lua_instance.globals().get::<usize>("line_yielded_at")
        else {
            return;
        };

        let before_current_line =
            source.split_inclusive("\n").take(current_line_index - 1).collect::<String>();
        let current_line =
            source.split_inclusive("\n").skip(current_line_index - 1).take(1).collect::<String>();
        let after_current_line =
            source.split_inclusive("\n").skip(current_line_index).collect::<String>();

        let mut job = LayoutJob::default();
        job.append(
            &before_current_line,
            0.,
            TextFormat {
                font_id: FontId { family: FontFamily::Monospace, size: 12. },
                ..Default::default()
            },
        );
        job.append(
            &current_line,
            0.,
            TextFormat {
                color: Color32::RED,
                font_id: FontId { family: FontFamily::Monospace, size: 12. },
                ..Default::default()
            },
        );
        job.append(
            &after_current_line,
            0.,
            TextFormat {
                font_id: FontId { family: FontFamily::Monospace, size: 12. },
                ..Default::default()
            },
        );

        Window::new(f!("Script: {}", &self.name()))
            .id(self.window_id)
            .open(&mut self.open)
            .default_width(500.)
            .show(&ctx, |ui| {
                ScrollArea::vertical().show(ui, |ui| {
                    if self.name.is_some() {
                        ui.label(serde_json::to_string(&self.script_id).expect(""));
                    }

                    ui.label(job);

                    // Pad if below window.default_width
                    ui.allocate_space([ui.available_width(), 0.].into());
                });
            });
    }

    fn name(&self) -> String {
        match &self.name {
            Some(name) => name.clone(),
            None => serde_json::to_string(&self.script_id).expect(""),
        }
    }
}

struct StoryVarsWindow {
    open: bool,
    filter_string: String,
    var_being_edited: Option<String>,
    edit_text: String,
}

impl StoryVarsWindow {
    fn new() -> Self {
        Self {
            open: false,
            filter_string: String::new(),
            var_being_edited: None,
            edit_text: String::new(),
        }
    }

    fn show(&mut self, ctx: &Context, story_vars: &mut StoryVars) {
        if !self.open {
            return;
        }

        Window::new("Story Vars").open(&mut self.open).default_width(250.).show(&ctx, |ui| {
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
