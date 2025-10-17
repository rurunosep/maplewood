use crate::ConsoleCommandExecutor;
use egui::{
    CentralPanel, Context, FontFamily, Key, Label, RichText, ScrollArea, TextEdit, TextStyle,
    TopBottomPanel, Widget, Window,
};
use std::collections::VecDeque;
use std::format as f;

pub struct ConsoleWindow {
    pub open: bool,
    input_panel: InputPanel,
    scrollback: String,
}

impl ConsoleWindow {
    pub fn show(&mut self, ctx: &Context, command_executor: &mut ConsoleCommandExecutor) {
        if !self.open {
            return;
        }

        // Get console command execution output
        for output in command_executor.output_queue.drain(..) {
            Self::push_to_scrollback(&mut self.scrollback, &output);
        }

        Window::new("Console")
            .open(&mut self.open)
            .default_width(800.)
            .default_height(400.)
            .show(&ctx, |ui| {
                // Input Line
                self.input_panel.show(ui, &mut self.scrollback, command_executor);

                // Scrollback
                CentralPanel::default().show_inside(ui, |ui| {
                    ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
                        // TODO colored console text
                        Label::new(RichText::new(&self.scrollback).family(FontFamily::Monospace))
                            .ui(ui);

                        ui.allocate_space([ui.available_width(), 0.].into());
                    })
                })
            });
    }

    pub fn new() -> Self {
        Self { open: false, input_panel: InputPanel::new(), scrollback: String::new() }
    }

    fn push_to_scrollback(scrollback: &mut String, str: &str) {
        if !scrollback.is_empty() {
            scrollback.push('\n');
        }
        scrollback.push_str(str);
    }
}

struct InputPanel {
    input_text: String,
    input_history: VecDeque<String>,
    history_cursor: Option<usize>,
}

impl InputPanel {
    fn show(
        &mut self,
        ui: &mut egui::Ui,
        scrollback: &mut String,
        console: &mut ConsoleCommandExecutor,
    ) {
        TopBottomPanel::bottom("input_panel").show_inside(ui, |ui| {
            let mut history_text =
                self.history_cursor.and_then(|i| self.input_history.get(i)).map(|s| s.clone());

            let textedit_ref = history_text.as_mut().unwrap_or(&mut self.input_text);

            let mut output = TextEdit::singleline(textedit_ref)
                .desired_width(f32::INFINITY)
                .font(TextStyle::Monospace)
                .return_key(None)
                .frame(false)
                .show(ui);

            let mut should_move_cursor_to_end = false;

            if output.response.has_focus() {
                ui.input(|i| {
                    for event in &i.events {
                        // Update input history scrolling
                        match event {
                            // Move history cursor up
                            egui::Event::Key { key: Key::ArrowUp, pressed: true, .. } => {
                                self.history_cursor = match self.history_cursor {
                                    Some(index) if index + 1 < self.input_history.len() => {
                                        Some(index + 1)
                                    }
                                    Some(index) => Some(index),
                                    None if !self.input_history.is_empty() => Some(0),
                                    None => None,
                                };

                                should_move_cursor_to_end = true;
                            }
                            // Move history cursor down
                            egui::Event::Key { key: Key::ArrowDown, pressed: true, .. } => {
                                self.history_cursor = match self.history_cursor {
                                    Some(index) if index > 0 => Some(index - 1),
                                    Some(_) => None,
                                    None => None,
                                };

                                should_move_cursor_to_end = true;
                            }
                            // If any key other than Up/Down was pressed, stop scrolling history
                            egui::Event::Key { pressed: true, .. } => {
                                if let Some(history_text) = &history_text {
                                    self.input_text = history_text.clone();
                                };
                                self.history_cursor = None;
                            }
                            _ => {}
                        }

                        // After updating history scrolling
                        match event {
                            egui::Event::Key { key: Key::Enter, pressed: true, .. } => {
                                // Add input to console command queue and scrollback
                                ConsoleWindow::push_to_scrollback(
                                    scrollback,
                                    &f!("> {}", &self.input_text),
                                );
                                console.input_queue.push(self.input_text.clone());

                                // Add input to beginning of history
                                self.input_history.retain(|s| *s != self.input_text);
                                self.input_history.push_front(self.input_text.clone());

                                self.input_text.clear();
                            }
                            _ => {}
                        }
                    }
                })
            }

            // If history was scrolled, the cursor needs to be moved to the end of the new text
            if should_move_cursor_to_end {
                output.state.cursor.set_char_range(Some(egui::text::CCursorRange::one(
                    egui::text::CCursor::new(
                        self.history_cursor
                            .and_then(|i| self.input_history.get(i))
                            .unwrap_or(&self.input_text)
                            .chars()
                            .count(),
                    ),
                )));
                output.state.store(ui.ctx(), output.response.id);
                // TextEdit has already been drawn with the old cursor position. Redraw the UI.
                ui.ctx().request_discard("moved textedit cursor");
            }
        });
    }

    fn new() -> Self {
        Self { input_text: String::new(), input_history: VecDeque::new(), history_cursor: None }
    }
}
