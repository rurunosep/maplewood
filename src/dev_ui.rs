use crate::components;
use crate::ecs::Ecs;
use sdl2::video::Window;
use std::time::Instant;

pub struct DevUiData<'window> {
    pub ctx: egui::Context,
    pub state: egui_sdl2_event::EguiSDL2State,
    pub window: &'window Window,
    pub active: bool,
    // Stored intermediately between processing and rendering for convenience
    pub full_output: Option<egui::FullOutput>,
    //
    pub player_position_window: Option<PlayerPositionWindow>,
}

impl DevUiData<'_> {
    // Keeps egui context zoom_factor and egui state dpi_scaling in sync
    pub fn set_zoom_factor(&mut self, zoom_factor: f32) {
        self.ctx.set_zoom_factor(zoom_factor);
        self.state.dpi_scaling = zoom_factor;
    }
}

// NOW list some entities and components in egui

// Show egui, process output and app state updates (nothing for now), and save intermediate
// full_output for rendering later
// (Eventually move to a debug_ui module)
pub fn run_dev_ui(
    dev_ui_data: &mut DevUiData<'_>,
    start_time: &Instant,
    //
    frame_duration: f32,
    ecs: &Ecs,
) {
    if !dev_ui_data.active {
        return;
    }

    let DevUiData { state, ctx, window, .. } = dev_ui_data;

    state.update_time(Some(start_time.elapsed().as_secs_f64()), 1. / 60.);
    ctx.begin_pass(state.raw_input.take());

    egui::Window::new("Debug").show(&ctx, |ui| {
        ui.label(format!("Frame Duration: {frame_duration:.2}%"));

        let mut is_open = dev_ui_data.player_position_window.is_some();
        ui.toggle_value(&mut is_open, "Player Position");
        match (is_open, &dev_ui_data.player_position_window) {
            (true, None) => {
                dev_ui_data.player_position_window = Some(PlayerPositionWindow::new(&ecs))
            }
            (false, Some(_)) => dev_ui_data.player_position_window = None,
            _ => {}
        };
    });

    if let Some(window) = &mut dev_ui_data.player_position_window {
        window.show(ctx);
    }

    let full_output = ctx.end_pass();
    // (Looks like this just updates the cursor and the clipboard text)
    state.process_output(window, &full_output.platform_output);
    dev_ui_data.full_output = Some(full_output);
}

pub struct PlayerPositionWindow {
    pub text: String,
}

impl PlayerPositionWindow {
    fn new(ecs: &Ecs) -> Self {
        Self {
            text: serde_json::to_string_pretty(
                &*ecs.query_one_with_name::<&components::Position>("player").unwrap(),
            )
            .unwrap(),
        }
    }

    fn show(&mut self, ctx: &egui::Context) {
        egui::Window::new("Player Position").show(&ctx, |ui| {
            ui.text_edit_multiline(&mut self.text);
        });
    }
}
