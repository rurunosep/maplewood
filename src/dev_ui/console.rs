use crate::ConsoleCommandExecutor;
use crate::misc::LOGGER;
use egui::{
    CentralPanel, Context, FontFamily, Key, Label, RichText, ScrollArea, TextEdit, TextStyle,
    TopBottomPanel, Widget, Window,
};
use std::collections::VecDeque;
use std::format as f;

pub struct ConsoleWindow {
    pub open: bool,
    scrollback: String,
    input_text: String,
    input_history: VecDeque<String>,
    history_cursor: Option<usize>,
}

impl ConsoleWindow {
    pub fn show(&mut self, ctx: &Context, console: &mut ConsoleCommandExecutor) {
        if !self.open {
            return;
        }

        // Get new console command output
        for output in console.output_queue.drain(..) {
            Self::push_to_scrollback(&mut self.scrollback, &output);
        }

        // Get new logs
        {
            let mut new_logs = LOGGER.to_console_queue.lock().unwrap();
            for new_log in new_logs.drain(..) {
                Self::push_to_scrollback(&mut self.scrollback, &new_log);
            }
        }

        Window::new("Console").open(&mut self.open).default_width(800.).show(&ctx, |ui| {
            // Input
            TopBottomPanel::bottom("bottom").show_inside(ui, |ui| {
                let mut history_text = self
                    .history_cursor
                    .and_then(|i| self.input_history.get(i))
                    .map(|s| s.clone());

                let text_edit_ref = if let Some(history_text) = &mut history_text {
                    history_text
                } else {
                    &mut self.input_text
                };

                // TODO fix TextEdit up and down key bullshit
                // TextEdit moves the cursor to beginning on Up and to end on Down
                // Disable that shit somehow
                // Context::request_discard

                let response = TextEdit::singleline(text_edit_ref)
                    .desired_width(f32::INFINITY)
                    .font(TextStyle::Monospace)
                    .return_key(None)
                    .id("t".into())
                    .frame(false)
                    .ui(ui);

                if response.has_focus() {
                    ui.input(|i| {
                        for event in &i.events {
                            // Update history scrolling
                            match event {
                                // A key was pressed. Was it Up, Down, or anything else?
                                egui::Event::Key { key, pressed: true, .. } => match key {
                                    Key::ArrowUp => {
                                        // Move history cursor up
                                        self.history_cursor = match self.history_cursor {
                                            Some(index)
                                                if index + 1 < self.input_history.len() =>
                                            {
                                                Some(index + 1)
                                            }
                                            Some(index) => Some(index),
                                            None => Some(0),
                                        };
                                    }
                                    Key::ArrowDown => {
                                        // Move history cursor down
                                        self.history_cursor = match self.history_cursor {
                                            Some(index) if index > 0 => Some(index - 1),
                                            Some(_) => None,
                                            None => None,
                                        }
                                    }
                                    _ => {
                                        // If any key other than Up or Down was pressed, stop
                                        // scrolling history
                                        if let Some(history_text) = &history_text {
                                            self.input_text = history_text.clone();
                                        };
                                        self.history_cursor = None;
                                    }
                                },
                                _ => {}
                            }

                            // After updating history scrolling
                            match event {
                                egui::Event::Key { key: Key::Enter, pressed: true, .. } => {
                                    // Add input to console command queue and scrollback
                                    Self::push_to_scrollback(
                                        &mut self.scrollback,
                                        &f!("> {}", &self.input_text),
                                    );
                                    console.input_queue.push(self.input_text.clone());

                                    // Add input to beginning of history
                                    self.input_history.retain(|s| *s != self.input_text);
                                    self.input_history.push_front(self.input_text.clone());

                                    // Clear input
                                    self.input_text.clear();
                                }
                                _ => {}
                            }
                        }
                    })
                }
            });

            // Log
            CentralPanel::default().show_inside(ui, |ui| {
                ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
                    Label::new(RichText::new(&*self.scrollback).family(FontFamily::Monospace))
                        .ui(ui);

                    ui.allocate_space([ui.available_width(), 0.].into());
                })
            })
        });
    }

    pub fn new() -> Self {
        Self {
            open: false,
            scrollback: String::new(),
            input_text: String::new(),
            input_history: VecDeque::new(),
            history_cursor: None,
        }
    }

    fn push_to_scrollback(scrollback: &mut String, str: &str) {
        if !scrollback.is_empty() {
            scrollback.push('\n');
        }
        scrollback.push_str(str);
    }
}
