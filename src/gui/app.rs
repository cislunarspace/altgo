//! eframe application — renders the GUI window.

use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui;
use egui::{Color32, FontId, RichText, TopBottomPanel, ViewportCommand};

use super::i18n;
use super::state::{GuiState, RecordingState, SharedState};

/// Main eframe application.
pub struct AltgoApp {
    state: Arc<SharedState>,
    last_state: RecordingState,
    last_transcription: Option<String>,
    /// Path to the config file for saving.
    config_path: PathBuf,
    /// Full config — cloned from and saved to disk.
    config: crate::config::Config,
    /// Whether the settings panel is open.
    settings_open: bool,
    /// Current UI language (takes effect immediately).
    lang: i18n::Lang,
    /// Edited config values (separate from actual config until saved).
    edit_key_name: String,
    edit_language: String,
    edit_engine: String,
    edit_api_key: String,
    edit_api_base_url: String,
    edit_model: String,
    edit_polisher_level: String,
    edit_polisher_api_key: String,
    edit_api_base_url_polisher: String,
    edit_polisher_model: String,
    edit_gui_language: i18n::Lang,
}

impl AltgoApp {
    pub fn new(state: Arc<SharedState>, config_path: PathBuf, cfg: crate::config::Config) -> Self {
        let lang = i18n::Lang::from_code(&cfg.gui.language);

        Self {
            state,
            last_state: RecordingState::Idle,
            last_transcription: None,
            config_path,
            config: cfg.clone(),
            settings_open: false,
            lang,
            edit_key_name: cfg.key_listener.key_name.clone(),
            edit_language: cfg.transcriber.language.clone(),
            edit_engine: cfg.transcriber.engine.clone(),
            edit_api_key: cfg.transcriber.api_key.clone(),
            edit_api_base_url: cfg.transcriber.api_base_url.clone(),
            edit_model: cfg.transcriber.model.clone(),
            edit_polisher_level: cfg.polisher.level.clone(),
            edit_polisher_api_key: cfg.polisher.api_key.clone(),
            edit_api_base_url_polisher: cfg.polisher.api_base_url.clone(),
            edit_polisher_model: cfg.polisher.model.clone(),
            edit_gui_language: lang,
        }
    }

    /// Save the edited config to disk.
    fn save_config(&mut self) {
        // Validate input values before saving.
        let valid_polisher_levels = ["none", "light", "medium", "heavy"];
        if !valid_polisher_levels.contains(&self.edit_polisher_level.as_str()) {
            tracing::warn!(
                level = %self.edit_polisher_level,
                "invalid polisher level, ignoring save"
            );
            self.state
                .set_message(i18n::t("settings.error_invalid_polish_level", self.lang));
            return;
        }

        let valid_engines = ["api", "local"];
        if !valid_engines.contains(&self.edit_engine.as_str()) {
            tracing::warn!(
                engine = %self.edit_engine,
                "invalid engine, ignoring save"
            );
            self.state
                .set_message(i18n::t("settings.error_invalid_engine", self.lang));
            return;
        }

        let mut cfg = self.config.clone();
        cfg.key_listener.key_name = self.edit_key_name.clone();
        cfg.transcriber.engine = self.edit_engine.clone();
        cfg.transcriber.api_key = self.edit_api_key.clone();
        cfg.transcriber.api_base_url = self.edit_api_base_url.clone();
        cfg.transcriber.model = self.edit_model.clone();
        cfg.transcriber.language = self.edit_language.clone();
        cfg.polisher.engine = "openai".to_string();
        cfg.polisher.api_key = self.edit_polisher_api_key.clone();
        cfg.polisher.api_base_url = self.edit_api_base_url_polisher.clone();
        cfg.polisher.model = self.edit_polisher_model.clone();
        cfg.polisher.level = self.edit_polisher_level.clone();
        cfg.gui.language = self.edit_gui_language.code().to_string();

        if let Err(e) = cfg.save(&self.config_path) {
            tracing::error!(error = %e, "failed to save config");
        } else {
            tracing::info!("settings saved");
            self.config = cfg;
        }
    }

    /// Helper to translate a key with the current language.
    fn t(&self, key: &'static str) -> &'static str {
        i18n::t(key, self.lang)
    }
}

impl eframe::App for AltgoApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let state = self.state.get();

        // Request repaint when state changes so we update promptly.
        if state.recording != self.last_state || state.transcription != self.last_transcription {
            ctx.request_repaint();
            self.last_state = state.recording;
            self.last_transcription = state.transcription.clone();
        }

        // Top area: menu bar + title.
        let lang = self.lang;
        TopBottomPanel::top("top").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                if ui.button(i18n::t("menu.settings", lang)).clicked() {
                    self.settings_open = !self.settings_open;
                }
                ui.menu_button(i18n::t("menu.file", lang), |ui| {
                    if ui.button(i18n::t("menu.exit", lang)).clicked() {
                        ctx.send_viewport_cmd(ViewportCommand::Close);
                    }
                });
                ui.menu_button(i18n::t("menu.help", lang), |ui| {
                    if ui.button(i18n::t("menu.about", lang)).clicked() {
                        ui.label(
                            RichText::new(i18n::t("about.text", lang))
                                .font(FontId::proportional(13.0)),
                        );
                    }
                });
            });

            // Title row.
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("altgo")
                        .font(FontId::proportional(22.0))
                        .color(Color32::from_rgb(0x4A, 0x9F, 0xFF)),
                );
                ui.add_space(8.0);
                ui.label(
                    RichText::new(i18n::t("title.subtitle", lang))
                        .font(FontId::proportional(13.0))
                        .color(Color32::GRAY),
                );
            });
            ui.add_space(4.0);
        });

        // Bottom status bar with animated indicator.
        TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                let (dot, color) = match state.recording {
                    RecordingState::Idle => ("○", Color32::GRAY),
                    RecordingState::Recording => ("●", Color32::RED),
                    RecordingState::Processing => ("◐", Color32::from_rgb(0xFF, 0xCC, 0x00)),
                    RecordingState::Done => ("✓", Color32::from_rgb(0x4A, 0xFF, 0x9F)),
                };
                ui.label(
                    RichText::new(dot)
                        .font(FontId::proportional(20.0))
                        .color(color),
                );
                ui.add_space(8.0);

                let status_text = match state.recording {
                    RecordingState::Idle => i18n::t("status.idle", lang),
                    RecordingState::Recording => i18n::t("status.recording", lang),
                    RecordingState::Processing => i18n::t("status.processing", lang),
                    RecordingState::Done => i18n::t("status.done", lang),
                };
                ui.label(RichText::new(status_text).font(FontId::proportional(14.0)));
            });
        });

        // Main content area.
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.settings_open {
                self.show_settings_ui(ui);
            } else {
                self.show_main_ui(ui, &state);
            }
        });

        // Continuous repaint during active states for animated indicators.
        if matches!(
            state.recording,
            RecordingState::Recording | RecordingState::Processing
        ) {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
    }
}

impl AltgoApp {
    fn show_main_ui(&self, ui: &mut egui::Ui, state: &GuiState) {
        ui.add_space(24.0);

        match state.recording {
            RecordingState::Idle | RecordingState::Recording | RecordingState::Processing => {
                ui.vertical_centered(|ui| {
                    let (emoji, emoji_color) = match state.recording {
                        RecordingState::Idle => ("🎤", Color32::from_rgb(0xAA, 0xAA, 0xAA)),
                        RecordingState::Recording => ("🔴", Color32::from_rgb(0xFF, 0x55, 0x55)),
                        _ => ("⚙️", Color32::from_rgb(0xFF, 0xCC, 0x00)),
                    };
                    ui.label(
                        RichText::new(emoji)
                            .font(FontId::proportional(72.0))
                            .color(emoji_color),
                    );
                    ui.add_space(16.0);

                    let main_text = match state.recording {
                        RecordingState::Idle => self.t("main.idle"),
                        RecordingState::Recording => self.t("main.recording"),
                        _ => self.t("main.processing"),
                    };
                    ui.label(
                        RichText::new(main_text)
                            .font(FontId::proportional(20.0))
                            .color(Color32::from_rgb(0xEE, 0xEE, 0xEE)),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(self.t("main.hint"))
                            .font(FontId::proportional(13.0))
                            .color(Color32::from_rgb(0x88, 0x88, 0x88)),
                    );
                });
            }
            RecordingState::Done => {
                if let Some(text) = &state.transcription {
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new(self.t("main.result_label"))
                                .font(FontId::proportional(11.0))
                                .color(Color32::from_rgb(0x66, 0x66, 0x66)),
                        );
                        ui.add_space(8.0);

                        // Scrollable text area with dark background.
                        egui::ScrollArea::vertical()
                            .max_height(180.0)
                            .show(ui, |ui| {
                                egui::Frame::default()
                                    .fill(Color32::from_rgb(0x18, 0x18, 0x18))
                                    .rounding(10.0)
                                    .inner_margin(14.0)
                                    .show(ui, |ui| {
                                        ui.label(
                                            RichText::new(text.as_str())
                                                .font(FontId::proportional(15.0))
                                                .line_height(Some(22.0))
                                                .color(Color32::from_rgb(0xF0, 0xF0, 0xF0)),
                                        );
                                    });
                            });

                        ui.add_space(12.0);
                        ui.label(
                            RichText::new(self.t("main.copied"))
                                .font(FontId::proportional(12.0))
                                .color(Color32::from_rgb(0x4A, 0xFF, 0x9F)),
                        );
                    });
                }
            }
        }
    }

    fn show_settings_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading(self.t("settings.title"));

        ui.add_space(8.0);

        egui::ScrollArea::vertical()
            .max_height(300.0)
            .show(ui, |ui| {
                ui.set_width(400.0);

                // UI language (at the top for discoverability).
                ui.horizontal(|ui| {
                    ui.label(self.t("settings.gui_language"));
                    let selected_label = match self.edit_gui_language {
                        i18n::Lang::Zh => "中文",
                        i18n::Lang::En => "English",
                    };
                    egui::ComboBox::from_id_salt("gui_language")
                        .selected_text(selected_label)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.edit_gui_language,
                                i18n::Lang::Zh,
                                "中文",
                            );
                            ui.selectable_value(
                                &mut self.edit_gui_language,
                                i18n::Lang::En,
                                "English",
                            );
                        });
                });
                ui.add_space(12.0);

                // Recording settings
                ui.label(
                    RichText::new(self.t("settings.recording"))
                        .font(FontId::proportional(14.0))
                        .color(Color32::from_rgb(0x4A, 0x9F, 0xFF)),
                );
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.label(self.t("settings.key_name"));
                    ui.text_edit_singleline(&mut self.edit_key_name);
                });
                ui.add_space(12.0);

                // Transcription settings
                ui.label(
                    RichText::new(self.t("settings.transcription"))
                        .font(FontId::proportional(14.0))
                        .color(Color32::from_rgb(0x4A, 0x9F, 0xFF)),
                );
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.label(self.t("settings.engine"));
                    let engine_api = i18n::t("settings.engine_api", self.lang);
                    let engine_local = i18n::t("settings.engine_local", self.lang);
                    egui::ComboBox::from_id_salt("engine")
                        .selected_text(&self.edit_engine)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.edit_engine,
                                "api".to_string(),
                                engine_api,
                            );
                            ui.selectable_value(
                                &mut self.edit_engine,
                                "local".to_string(),
                                engine_local,
                            );
                        });
                });

                ui.horizontal(|ui| {
                    ui.label(self.t("settings.language"));
                    ui.text_edit_singleline(&mut self.edit_language);
                });

                if self.edit_engine == "api" {
                    ui.horizontal(|ui| {
                        ui.label(self.t("settings.api_key"));
                        ui.add(egui::TextEdit::singleline(&mut self.edit_api_key).password(true));
                    });

                    ui.horizontal(|ui| {
                        ui.label(self.t("settings.api_url"));
                        ui.text_edit_singleline(&mut self.edit_api_base_url);
                    });

                    ui.horizontal(|ui| {
                        ui.label(self.t("settings.model"));
                        ui.text_edit_singleline(&mut self.edit_model);
                    });
                } else {
                    ui.horizontal(|ui| {
                        ui.label(self.t("settings.model_path"));
                        ui.text_edit_singleline(&mut self.edit_model);
                    });
                }
                ui.add_space(12.0);

                // Polisher settings
                ui.label(
                    RichText::new(self.t("settings.polishing"))
                        .font(FontId::proportional(14.0))
                        .color(Color32::from_rgb(0x4A, 0x9F, 0xFF)),
                );
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.label(self.t("settings.polish_level"));
                    let p_none = i18n::t("settings.polish_none", self.lang);
                    let p_light = i18n::t("settings.polish_light", self.lang);
                    let p_medium = i18n::t("settings.polish_medium", self.lang);
                    let p_heavy = i18n::t("settings.polish_heavy", self.lang);
                    egui::ComboBox::from_id_salt("polish_level")
                        .selected_text(&self.edit_polisher_level)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.edit_polisher_level,
                                "none".to_string(),
                                p_none,
                            );
                            ui.selectable_value(
                                &mut self.edit_polisher_level,
                                "light".to_string(),
                                p_light,
                            );
                            ui.selectable_value(
                                &mut self.edit_polisher_level,
                                "medium".to_string(),
                                p_medium,
                            );
                            ui.selectable_value(
                                &mut self.edit_polisher_level,
                                "heavy".to_string(),
                                p_heavy,
                            );
                        });
                });

                ui.horizontal(|ui| {
                    ui.label(self.t("settings.api_url"));
                    ui.text_edit_singleline(&mut self.edit_api_base_url_polisher);
                });

                ui.horizontal(|ui| {
                    ui.label(self.t("settings.api_key"));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.edit_polisher_api_key).password(true),
                    );
                });

                ui.horizontal(|ui| {
                    ui.label(self.t("settings.model"));
                    ui.text_edit_singleline(&mut self.edit_polisher_model);
                });
            });

        ui.add_space(12.0);

        // Show message if one is set.
        let state = self.state.get();
        if let Some(msg) = &state.message {
            ui.label(
                RichText::new(msg.as_str())
                    .font(FontId::proportional(12.0))
                    .color(Color32::RED),
            );
        }
        drop(state);

        ui.horizontal(|ui| {
            if ui.button(self.t("settings.save")).clicked() {
                // Apply language change immediately.
                self.lang = self.edit_gui_language;
                self.save_config();
            }
            if ui.button(self.t("settings.cancel")).clicked() {
                // Restore edit fields from stored config.
                self.edit_key_name = self.config.key_listener.key_name.clone();
                self.edit_language = self.config.transcriber.language.clone();
                self.edit_engine = self.config.transcriber.engine.clone();
                self.edit_api_key = self.config.transcriber.api_key.clone();
                self.edit_api_base_url = self.config.transcriber.api_base_url.clone();
                self.edit_model = self.config.transcriber.model.clone();
                self.edit_polisher_level = self.config.polisher.level.clone();
                self.edit_polisher_api_key = self.config.polisher.api_key.clone();
                self.edit_api_base_url_polisher = self.config.polisher.api_base_url.clone();
                self.edit_polisher_model = self.config.polisher.model.clone();
                self.edit_gui_language = i18n::Lang::from_code(&self.config.gui.language);
                self.settings_open = false;
            }
        });

        ui.add_space(8.0);
        ui.label(
            RichText::new(self.t("settings.restart_hint"))
                .font(FontId::proportional(11.0))
                .color(Color32::GRAY),
        );
    }
}
