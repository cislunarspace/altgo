//! eframe application — renders the GUI window.

use std::sync::Arc;

use eframe::egui;
use egui::{Color32, FontId, RichText, TopBottomPanel, ViewportCommand};

use super::state::{RecordingState, SharedState};

/// Main eframe application.
pub struct AltgoApp {
    state: Arc<SharedState>,
    last_state: RecordingState,
    last_transcription: Option<String>,
}

impl AltgoApp {
    pub fn new(state: Arc<SharedState>) -> Self {
        Self {
            state,
            last_state: RecordingState::Idle,
            last_transcription: None,
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

        // Top area: menu bar + title.
        TopBottomPanel::top("top").show(ctx, |ui| {
            // Menu bar.
            egui::menu::bar(ui, |ui| {
                ui.menu_button("文件", |ui| {
                    if ui.button("退出").clicked() {
                        ctx.send_viewport_cmd(ViewportCommand::Close);
                    }
                });
                ui.menu_button("帮助", |ui| {
                    if ui.button("关于 altgo").clicked() {
                        // Simple about dialog.
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

        // Central content area.
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(24.0);

            match state.recording {
                RecordingState::Idle | RecordingState::Recording | RecordingState::Processing => {
                    // Idle / Recording UI.
                    ui.vertical_centered(|ui| {
                        let (emoji, emoji_color) = match state.recording {
                            RecordingState::Idle => ("🎤", Color32::from_rgb(0xAA, 0xAA, 0xAA)),
                            RecordingState::Recording => {
                                ("🔴", Color32::from_rgb(0xFF, 0x55, 0x55))
                            }
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
                    // Transcription result UI.
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
