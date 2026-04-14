//! eframe application — renders the GUI window.

use std::path::PathBuf;
use std::sync::Arc;

use eframe::egui;
use egui::{Color32, FontId, RichText, TopBottomPanel, ViewportCommand};

use super::state::{GuiState, RecordingState, SharedState};

/// Main eframe application.
pub struct AltgoApp {
    state: Arc<SharedState>,
    last_state: RecordingState,
    last_transcription: Option<String>,
    /// Path to the config file for saving.
    config_path: PathBuf,
    /// Whether the settings panel is open.
    settings_open: bool,
    /// Edited config values (separate from actual config until saved).
    edit_key_name: String,
    edit_language: String,
    edit_engine: String,
    edit_api_key: String,
    edit_api_base_url: String,
    edit_model: String,
    edit_polisher_level: String,
    edit_api_base_url_polisher: String,
    edit_polisher_model: String,
}

impl AltgoApp {
    pub fn new(state: Arc<SharedState>, config_path: PathBuf) -> Self {
        // Load current config for editing.
        let cfg = crate::config::Config::load(&config_path)
            .unwrap_or_else(|_| crate::config::Config::default());

        Self {
            state,
            last_state: RecordingState::Idle,
            last_transcription: None,
            config_path,
            settings_open: false,
            edit_key_name: cfg.key_listener.key_name,
            edit_language: cfg.transcriber.language.clone(),
            edit_engine: cfg.transcriber.engine.clone(),
            edit_api_key: cfg.transcriber.api_key.clone(),
            edit_api_base_url: cfg.transcriber.api_base_url.clone(),
            edit_model: cfg.transcriber.model.clone(),
            edit_polisher_level: cfg.polisher.level.clone(),
            edit_api_base_url_polisher: cfg.polisher.api_base_url.clone(),
            edit_polisher_model: cfg.polisher.model.clone(),
        }
    }

    /// Save the edited config to disk.
    fn save_config(&mut self) {
        let cfg = crate::config::Config {
            key_listener: crate::config::KeyListenerConfig {
                key_name: self.edit_key_name.clone(),
                ..Default::default()
            },
            transcriber: crate::config::TranscriberConfig {
                engine: self.edit_engine.clone(),
                api_key: self.edit_api_key.clone(),
                api_base_url: self.edit_api_base_url.clone(),
                model: self.edit_model.clone(),
                language: self.edit_language.clone(),
                ..Default::default()
            },
            polisher: crate::config::PolisherConfig {
                engine: "openai".to_string(),
                api_key: self.edit_api_key.clone(),
                api_base_url: self.edit_api_base_url_polisher.clone(),
                model: self.edit_polisher_model.clone(),
                level: self.edit_polisher_level.clone(),
                ..Default::default()
            },
            ..Default::default()
        };

        if let Err(e) = cfg.save(&self.config_path) {
            tracing::error!(error = %e, "failed to save config");
        } else {
            tracing::info!("settings saved");
        }
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

        // Handle close request - hide window instead of closing.
        if ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }

        // Top area: menu bar + title.
        TopBottomPanel::top("top").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                if ui.button("设置").clicked() {
                    self.settings_open = !self.settings_open;
                }
                ui.menu_button("文件", |ui| {
                    if ui.button("退出").clicked() {
                        ctx.send_viewport_cmd(ViewportCommand::Close);
                    }
                });
                ui.menu_button("帮助", |ui| {
                    if ui.button("关于 altgo").clicked() {
                        ui.label(
                            RichText::new(
                                "altgo v0.1.0\n无需打字，言出法随\n按住 Alt 键说话，松开自动转写",
                            )
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
                    RichText::new("语音转文字")
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
                    RecordingState::Idle => "等待说话",
                    RecordingState::Recording => "正在录音...",
                    RecordingState::Processing => "正在转写...",
                    RecordingState::Done => "转写完成，已复制到剪贴板",
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
                        RecordingState::Idle => "按住 右 Alt 键说话",
                        RecordingState::Recording => "正在录音...",
                        _ => "正在处理...",
                    };
                    ui.label(
                        RichText::new(main_text)
                            .font(FontId::proportional(20.0))
                            .color(Color32::from_rgb(0xEE, 0xEE, 0xEE)),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new("松开后自动转写并复制到剪贴板")
                            .font(FontId::proportional(13.0))
                            .color(Color32::from_rgb(0x88, 0x88, 0x88)),
                    );
                });
            }
            RecordingState::Done => {
                if let Some(text) = &state.transcription {
                    ui.vertical_centered(|ui| {
                        ui.label(
                            RichText::new("转写结果")
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
                            RichText::new("已复制到剪贴板 ✓")
                                .font(FontId::proportional(12.0))
                                .color(Color32::from_rgb(0x4A, 0xFF, 0x9F)),
                        );
                    });
                }
            }
        }
    }

    fn show_settings_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("设置");

        ui.add_space(8.0);

        egui::ScrollArea::vertical()
            .max_height(300.0)
            .show(ui, |ui| {
                ui.set_width(400.0);

                // Recording settings
                ui.label(
                    RichText::new("录音设置")
                        .font(FontId::proportional(14.0))
                        .color(Color32::from_rgb(0x4A, 0x9F, 0xFF)),
                );
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.label("按键名称:");
                    ui.text_edit_singleline(&mut self.edit_key_name);
                });
                ui.add_space(12.0);

                // Transcription settings
                ui.label(
                    RichText::new("转写设置")
                        .font(FontId::proportional(14.0))
                        .color(Color32::from_rgb(0x4A, 0x9F, 0xFF)),
                );
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.label("引擎:");
                    egui::ComboBox::from_id_salt("engine")
                        .selected_text(&self.edit_engine)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.edit_engine,
                                "api".to_string(),
                                "API (OpenAI兼容)",
                            );
                            ui.selectable_value(
                                &mut self.edit_engine,
                                "local".to_string(),
                                "本地 (whisper.cpp)",
                            );
                        });
                });

                ui.horizontal(|ui| {
                    ui.label("语言:");
                    ui.text_edit_singleline(&mut self.edit_language);
                });

                if self.edit_engine == "api" {
                    ui.horizontal(|ui| {
                        ui.label("API Key:");
                        ui.text_edit_singleline(&mut self.edit_api_key);
                    });

                    ui.horizontal(|ui| {
                        ui.label("API URL:");
                        ui.text_edit_singleline(&mut self.edit_api_base_url);
                    });

                    ui.horizontal(|ui| {
                        ui.label("模型:");
                        ui.text_edit_singleline(&mut self.edit_model);
                    });
                } else {
                    ui.horizontal(|ui| {
                        ui.label("模型路径:");
                        ui.text_edit_singleline(&mut self.edit_model);
                    });
                }
                ui.add_space(12.0);

                // Polisher settings
                ui.label(
                    RichText::new("润色设置")
                        .font(FontId::proportional(14.0))
                        .color(Color32::from_rgb(0x4A, 0x9F, 0xFF)),
                );
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.label("润色级别:");
                    egui::ComboBox::from_id_salt("polish_level")
                        .selected_text(&self.edit_polisher_level)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.edit_polisher_level,
                                "none".to_string(),
                                "关闭",
                            );
                            ui.selectable_value(
                                &mut self.edit_polisher_level,
                                "light".to_string(),
                                "轻度",
                            );
                            ui.selectable_value(
                                &mut self.edit_polisher_level,
                                "medium".to_string(),
                                "中度",
                            );
                            ui.selectable_value(
                                &mut self.edit_polisher_level,
                                "heavy".to_string(),
                                "重度",
                            );
                        });
                });

                ui.horizontal(|ui| {
                    ui.label("API URL:");
                    ui.text_edit_singleline(&mut self.edit_api_base_url_polisher);
                });

                ui.horizontal(|ui| {
                    ui.label("模型:");
                    ui.text_edit_singleline(&mut self.edit_polisher_model);
                });
            });

        ui.add_space(12.0);

        ui.horizontal(|ui| {
            if ui.button("保存").clicked() {
                self.save_config();
            }
            if ui.button("取消").clicked() {
                self.settings_open = false;
            }
        });

        ui.add_space(8.0);
        ui.label(
            RichText::new("提示: 部分设置需要重启应用后生效")
                .font(FontId::proportional(11.0))
                .color(Color32::GRAY),
        );
    }
}
