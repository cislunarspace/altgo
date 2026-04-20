//! Tauri commands — 前端通过 IPC 调用的函数。

use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::Serialize;
use tauri::{Emitter, Manager, State};

use crate::{
    config, key_listener, output, pipeline, polisher, recorder, state_machine, transcriber,
    AppState,
};

const OVERLAY_RECORDING_W: f64 = 200.0;
const OVERLAY_RECORDING_H: f64 = 48.0;
const OVERLAY_RESULT_W: f64 = 520.0;
const OVERLAY_RESULT_H: f64 = 100.0;
const OVERLAY_BOTTOM_OFFSET: f64 = 80.0;

#[cfg(target_os = "windows")]
#[link(name = "user32")]
extern "system" {
    fn GetAsyncKeyState(vKey: i32) -> i16;
}

#[cfg(target_os = "windows")]
fn is_key_pressed(vk: i32) -> bool {
    // SAFETY: GetAsyncKeyState is a thread-safe Win32 API. The vKey parameter
    // is a valid virtual-key code produced by resolve_vk_code. No pointers or
    // mutable state are involved.
    unsafe { (GetAsyncKeyState(vk) as u16 & 0x8000) != 0 }
}

fn resolve_vk_code(key_name: &str) -> Result<i32, String> {
    match key_name {
        "ISO_Level3_Shift" | "Alt_R" | "RightAlt" => Ok(0xA5),
        "Alt_L" | "LeftAlt" => Ok(0xA4),
        "Super_L" | "Win_L" => Ok(0x5B),
        "Super_R" | "Win_R" => Ok(0x5C),
        "Control_R" => Ok(0xA3),
        "Shift_R" => Ok(0xA1),
        _ => Err(format!("unsupported key: {}", key_name)),
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordingStatus {
    Idle,
    Recording,
    Processing,
    Done,
}

fn emit_pipeline_status(
    app: &tauri::AppHandle,
    status: &Arc<std::sync::RwLock<String>>,
    value: &str,
) {
    let _ = app.emit("pipeline-status", value);
    if let Ok(mut s) = status.write() {
        *s = value.to_string();
    }
}

/// Parse monitor geometry from xrandr --listmonitors output.
/// Format: " <idx>: +[*]<name> <W>/<pw>x<H>/<ph>+<X>+<Y>  <output>"
fn parse_monitor_geom(line: &str) -> Option<(f64, f64, f64, f64)> {
    // Find the geometry part: look for pattern like "6144/697x3456/392+3840+0"
    let line = line.trim_start();
    // Skip "N: +*name " prefix — find the geometry after the second space
    let mut spaces = 0;
    let mut geom_start = 0;
    for (i, c) in line.char_indices() {
        if c == ' ' {
            spaces += 1;
            if spaces == 2 {
                geom_start = i + 1;
                break;
            }
        }
    }
    if geom_start == 0 {
        return None;
    }

    // Find end of geometry (next space or end of line)
    let geom_end = line[geom_start..]
        .find(' ')
        .map(|p| geom_start + p)
        .unwrap_or(line.len());
    let geom = &line[geom_start..geom_end];

    // Parse "6144/697x3456/392+3840+0"
    let (w_part, rest) = geom.split_once('x')?;
    let w: f64 = w_part.split('/').next()?.parse().ok()?;

    // rest = "3456/392+3840+0"
    let parts: Vec<&str> = rest.split('+').collect();
    if parts.len() < 3 {
        return None;
    }
    let h: f64 = parts[0].split('/').next()?.parse().ok()?;
    let x: f64 = parts[1].parse().ok()?;
    let y: f64 = parts[2].parse().ok()?;

    Some((x, y, w, h))
}

/// Get monitor info for the screen where the mouse cursor is.
/// Returns (monitor_x, monitor_y, monitor_width, monitor_height).
#[cfg(target_os = "linux")]
fn get_focused_monitor_info() -> Option<(f64, f64, f64, f64)> {
    // Use mouse position to determine which monitor the user is on
    let mouse_out = std::process::Command::new("xdotool")
        .args(["getmouselocation", "--shell"])
        .output()
        .ok()?;

    if !mouse_out.status.success() {
        return None;
    }

    let mouse_str = String::from_utf8_lossy(&mouse_out.stdout);
    let mut mouse_x: i32 = 0;
    let mut mouse_y: i32 = 0;

    for line in mouse_str.lines() {
        if let Some(val) = line.strip_prefix("X=") {
            mouse_x = val.parse().ok()?;
        } else if let Some(val) = line.strip_prefix("Y=") {
            mouse_y = val.parse().ok()?;
        }
    }

    tracing::info!("mouse position: ({}, {})", mouse_x, mouse_y);

    // Parse xrandr --listmonitors for reliable monitor geometry
    let xrandr = std::process::Command::new("xrandr")
        .args(["--listmonitors"])
        .output()
        .ok()?;

    if !xrandr.status.success() {
        return None;
    }

    let xrandr_str = String::from_utf8_lossy(&xrandr.stdout);

    for line in xrandr_str.lines() {
        if let Some((m_x, m_y, m_w, m_h)) = parse_monitor_geom(line) {
            tracing::info!(
                "monitor: x={}, y={}, w={}, h={}",
                m_x, m_y, m_w, m_h
            );
            if mouse_x >= m_x as i32
                && mouse_x < (m_x + m_w) as i32
                && mouse_y >= m_y as i32
                && mouse_y < (m_y + m_h) as i32
            {
                tracing::info!("matched monitor at ({}, {})", m_x, m_y);
                return Some((m_x, m_y, m_w, m_h));
            }
        }
    }

    tracing::warn!("no monitor matched for mouse ({}, {})", mouse_x, mouse_y);
    None
}

#[cfg(target_os = "windows")]
fn get_focused_monitor_info() -> Option<(f64, f64, f64, f64)> {
    None
}

fn position_overlay(overlay: &tauri::WebviewWindow, width: f64, height: f64) {
    let _ = overlay.set_size(tauri::LogicalSize::new(width, height));

    if let Some((screen_x, screen_y, screen_w, screen_h)) = get_focused_monitor_info() {
        let x = screen_x + (screen_w - width) / 2.0;
        let y = screen_y + screen_h - height - OVERLAY_BOTTOM_OFFSET;
        let _ = overlay.set_position(tauri::LogicalPosition::new(x, y));
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigResponse {
    pub key_name: String,
    pub language: String,
    pub engine: String,
    pub model: String,
    pub api_base_url: String,
    pub polish_level: String,
    pub polish_model: String,
    pub polish_api_base_url: String,
    pub gui_language: String,
}

#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<ConfigResponse, String> {
    let cfg = state.config.lock().await;
    Ok(ConfigResponse {
        key_name: cfg.key_listener.key_name.clone(),
        language: cfg.transcriber.language.clone(),
        engine: cfg.transcriber.engine.clone(),
        model: cfg.transcriber.model.clone(),
        api_base_url: cfg.transcriber.api_base_url.clone(),
        polish_level: cfg.polisher.level.clone(),
        polish_model: cfg.polisher.model.clone(),
        polish_api_base_url: cfg.polisher.api_base_url.clone(),
        gui_language: cfg.gui.language.clone(),
    })
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveConfigRequest {
    pub key_name: Option<String>,
    pub language: Option<String>,
    pub engine: Option<String>,
    pub model: Option<String>,
    pub api_key: Option<String>,
    pub api_base_url: Option<String>,
    pub polish_level: Option<String>,
    pub polish_model: Option<String>,
    pub polish_api_key: Option<String>,
    pub polish_api_base_url: Option<String>,
    pub gui_language: Option<String>,
}

#[tauri::command]
pub async fn save_config(state: State<'_, AppState>, req: SaveConfigRequest) -> Result<(), String> {
    let mut cfg = state.config.lock().await;

    if let Some(v) = req.key_name {
        cfg.key_listener.key_name = v;
    }
    if let Some(v) = req.language {
        cfg.transcriber.language = v;
    }
    if let Some(v) = req.engine {
        cfg.transcriber.engine = v;
    }
    if let Some(v) = req.model {
        cfg.transcriber.model = v;
    }
    if let Some(v) = req.api_key {
        cfg.transcriber.api_key = v;
    }
    if let Some(v) = req.api_base_url {
        cfg.transcriber.api_base_url = v;
    }
    if let Some(v) = req.polish_level {
        cfg.polisher.level = v;
    }
    if let Some(v) = req.polish_model {
        cfg.polisher.model = v;
    }
    if let Some(v) = req.polish_api_key {
        cfg.polisher.api_key = v;
    }
    if let Some(v) = req.polish_api_base_url {
        cfg.polisher.api_base_url = v;
    }
    if let Some(v) = req.gui_language {
        cfg.gui.language = v;
    }

    cfg.save(&state.config_path)
        .map_err(|e| format!("save failed: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn start_pipeline(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut pipeline = state.pipeline.lock().await;
    if pipeline.is_some() {
        return Err("pipeline already running".into());
    }

    let cfg = state.config.lock().await.clone();
    let cfg = Arc::new(cfg);

    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    let pipeline_status = state.pipeline_status.clone();

    let app_clone = app.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to build tokio runtime");
        rt.block_on(run_pipeline(app_clone, cfg, stop_rx, pipeline_status));
    });

    *pipeline = Some(crate::PipelineHandle { stop_tx });
    Ok(())
}

#[tauri::command]
pub async fn stop_pipeline(state: State<'_, AppState>) -> Result<(), String> {
    let mut pipeline = state.pipeline.lock().await;
    if let Some(handle) = pipeline.take() {
        let _ = handle.stop_tx.send(());
    }
    Ok(())
}

#[tauri::command]
pub async fn get_status(state: State<'_, AppState>) -> Result<RecordingStatus, String> {
    let status = state
        .pipeline_status
        .read()
        .unwrap_or_else(|e| e.into_inner());
    match status.as_str() {
        "recording" => Ok(RecordingStatus::Recording),
        "processing" => Ok(RecordingStatus::Processing),
        "done" => Ok(RecordingStatus::Done),
        _ => Ok(RecordingStatus::Idle),
    }
}

#[tauri::command]
pub async fn copy_text(text: String) -> Result<(), String> {
    output::write_clipboard(&text)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn hide_overlay(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(overlay) = app.get_webview_window("overlay") {
        let _ = overlay.hide();
    }
    Ok(())
}

pub async fn run_pipeline(
    app: tauri::AppHandle,
    cfg: Arc<config::Config>,
    mut stop_rx: tokio::sync::oneshot::Receiver<()>,
    pipeline_status: Arc<std::sync::RwLock<String>>,
) {
    let vk_code = match resolve_vk_code(&cfg.key_listener.key_name) {
        Ok(vk) => vk,
        Err(e) => {
            tracing::error!(error = %e, "failed to resolve key code");
            let _ = app.emit("pipeline-error", format!("key resolve: {}", e));
            return;
        }
    };
    tracing::info!(
        "resolved key '{}' to VK code {}",
        cfg.key_listener.key_name,
        vk_code
    );

    let poll_interval_ms = cfg.key_listener.poll_interval_ms;

    let (raw_key_tx, raw_key_rx) = tokio::sync::mpsc::unbounded_channel();
    let poll_running = Arc::new(AtomicBool::new(true));

    #[cfg(target_os = "windows")]
    {
        let poll_running = poll_running.clone();
        std::thread::spawn(move || {
            let mut was_down = false;
            while poll_running.load(Ordering::SeqCst) {
                let is_down = is_key_pressed(vk_code);
                if is_down && !was_down {
                    let _ = raw_key_tx.send(key_listener::KeyEvent { pressed: true });
                } else if !is_down && was_down {
                    let _ = raw_key_tx.send(key_listener::KeyEvent { pressed: false });
                }
                was_down = is_down;
                std::thread::sleep(std::time::Duration::from_millis(poll_interval_ms));
            }
        });
    }

    #[cfg(not(target_os = "windows"))]
    {
        let mut listener =
            match key_listener::PlatformListener::new(&cfg.key_listener.key_name) {
                Ok(l) => l,
                Err(e) => {
                    tracing::error!(error = %e, "failed to create key listener");
                    let _ = app.emit("pipeline-error", format!("key listener: {}", e));
                    return;
                }
            };
        let mut key_events = match listener.start() {
            Ok(rx) => rx,
            Err(e) => {
                tracing::error!(error = %e, "failed to start key listener");
                let _ = app.emit("pipeline-error", format!("key listener start: {}", e));
                return;
            }
        };
        // Forward PlatformListener events into the shared raw_key channel.
        let poll_running = poll_running.clone();
        std::thread::spawn(move || {
            while poll_running.load(Ordering::SeqCst) {
                match key_events.try_recv() {
                    Ok(ev) => { let _ = raw_key_tx.send(ev); }
                    Err(_) => std::thread::sleep(std::time::Duration::from_millis(poll_interval_ms)),
                }
            }
            // Keep listener alive until the poll loop ends.
            drop(listener);
        });
    }

    let mut recorder =
        recorder::PlatformRecorder::new(cfg.recorder.sample_rate, cfg.recorder.channels);

    let model_path = if cfg.transcriber.engine == "local" {
        match altgo::model::resolve_model_path(&cfg.transcriber.model) {
            Some(p) => p.to_string_lossy().to_string(),
            None => {
                let msg = format!(
                    "本地模型未找到（配置值: {:?}）。请在 GUI 设置中下载模型，或将 [transcriber] model 设为已下载模型的名称（如 \"base\"）或完整文件路径。",
                    cfg.transcriber.model
                );
                tracing::error!("{}", msg);
                let _ = app.emit("pipeline-error", &msg);
                return;
            }
        }
    } else {
        cfg.transcriber.model.clone()
    };

    let transcriber: transcriber::Transcriber = match cfg.transcriber.engine.as_str() {
        "local" => transcriber::Transcriber::Local(transcriber::LocalWhisper::new(
            model_path,
            cfg.transcriber.language.clone(),
            cfg.transcriber.whisper_path.clone(),
        )),
        _ => match transcriber::WhisperApi::new(
            cfg.transcriber.api_key.clone(),
            cfg.transcriber.api_base_url.clone(),
            cfg.transcriber.model.clone(),
            cfg.transcriber.language.clone(),
            cfg.transcriber.temperature,
            cfg.transcriber.prompt.clone(),
            cfg.transcriber.timeout(),
        ) {
            Ok(api) => transcriber::Transcriber::Api(api),
            Err(e) => {
                tracing::error!(error = %e, "failed to create transcriber");
                let _ = app.emit("pipeline-error", format!("transcriber: {}", e));
                return;
            }
        },
    };

    let polish_level =
        polisher::PolishLevel::from_str(&cfg.polisher.level).unwrap_or_else(|_| {
            tracing::warn!("invalid polish level, using medium");
            polisher::PolishLevel::Medium
        });
    let polisher_protocol = polisher::ApiProtocol::from_str(&cfg.polisher.protocol)
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, "invalid polisher protocol, defaulting to openai");
            polisher::ApiProtocol::OpenAi
        });
    let formatter = match polisher::LLMFormatter::with_config(
        cfg.polisher.api_key.clone(),
        cfg.polisher.api_base_url.clone(),
        cfg.polisher.model.clone(),
        cfg.polisher.timeout(),
        cfg.polisher.max_tokens,
        polisher_protocol,
        cfg.polisher.temperature,
        cfg.transcriber.language.clone(),
        cfg.polisher.system_prompt.clone(),
    ) {
        Ok(f) => f,
        Err(e) => {
            tracing::error!(error = %e, "failed to create polisher");
            let _ = app.emit("pipeline-error", format!("polisher: {}", e));
            return;
        }
    };

    let (key_tx, key_rx) = tokio::sync::mpsc::unbounded_channel();
    let debounce_window = cfg.key_listener.debounce_window();
    tokio::spawn(key_listener::debounce_task(
        raw_key_rx,
        key_tx,
        debounce_window,
    ));

    let sm = state_machine::Machine::new(
        cfg.key_listener.long_press_threshold(),
        cfg.key_listener.double_click_interval(),
        cfg.key_listener.min_press_duration(),
    );
    let mut commands = sm.run(key_rx);

    emit_pipeline_status(&app, &pipeline_status, "idle");

    loop {
        tokio::select! {
            cmd = commands.recv() => {
                match cmd {
                    Some(state_machine::Command::StartRecord) => {
                        tracing::info!("recording started");
                        if let Err(e) = recorder.start() {
                            tracing::error!(error = %e, "failed to start recording");
                            continue;
                        }
                        emit_pipeline_status(&app, &pipeline_status, "recording");
                        if let Some(overlay) = app.get_webview_window("overlay") {
                            position_overlay(&overlay, OVERLAY_RECORDING_W, OVERLAY_RECORDING_H);
                            let _ = overlay.show();
                        }
                    }
                    Some(state_machine::Command::StopRecord) => {
                        tracing::info!("recording stopped, processing...");
                        if let Some(overlay) = app.get_webview_window("overlay") {
                            let _ = overlay.hide();
                        }
                        emit_pipeline_status(&app, &pipeline_status, "processing");
                        if let Some(overlay) = app.get_webview_window("overlay") {
                            position_overlay(&overlay, OVERLAY_RECORDING_W, OVERLAY_RECORDING_H);
                            let _ = overlay.show();
                        }
                        let wav_data = match recorder.stop() {
                            Ok(data) => data,
                            Err(e) => {
                                tracing::error!(error = %e, "failed to stop recording");
                                emit_pipeline_status(&app, &pipeline_status, "idle");
                                if let Some(overlay) = app.get_webview_window("overlay") {
                                    let _ = overlay.hide();
                                }
                                continue;
                            }
                        };

                        let cfg = Arc::clone(&cfg);
                        let formatter = formatter.clone();
                        let transcriber = transcriber.clone();
                        let app = app.clone();
                        let pipeline_status = pipeline_status.clone();

                        tokio::spawn(async move {
                            match pipeline::process_audio_core(
                                &transcriber,
                                &formatter,
                                &wav_data,
                                polish_level,
                            )
                            .await
                            {
                                Ok(output) => {
                                    if !output.text.is_empty() {
                                        let text_to_use = if cfg.output.prefer_polished
                                            && !output.polish_failed
                                        {
                                            &output.text
                                        } else {
                                            &output.raw_text
                                        };
                                        let display_text = text_to_use.clone();

                                        let _ =
                                            output::write_clipboard(text_to_use).await;

                                        emit_pipeline_status(&app, &pipeline_status, "done");
                                        if let Some(overlay) =
                                            app.get_webview_window("overlay")
                                        {
                                            position_overlay(
                                                &overlay,
                                                OVERLAY_RESULT_W,
                                                OVERLAY_RESULT_H,
                                            );
                                            let _ = overlay.show();
                                        }
                                        let _ =
                                            app.emit("transcription-result", &display_text);
                                    } else {
                                        emit_pipeline_status(&app, &pipeline_status, "idle");
                                    }
                                }
                                Err(e) => {
                                    tracing::error!(error = %e, "audio processing failed");
                                    let _ = app.emit("pipeline-error", format!("processing: {}", e));
                                    emit_pipeline_status(&app, &pipeline_status, "idle");
                                    if let Some(overlay) = app.get_webview_window("overlay") {
                                        let _ = overlay.hide();
                                    }
                                }
                            }
                        });
                    }
                    None => break,
                }
            }
            _ = &mut stop_rx => {
                tracing::info!("pipeline stop requested");
                poll_running.store(false, Ordering::SeqCst);
                break;
            }
        }
    }

    emit_pipeline_status(&app, &pipeline_status, "stopped");
    tracing::info!("pipeline stopped");
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelEntry {
    pub name: String,
    pub filename: String,
    pub size_bytes: u64,
    pub description: String,
    pub downloaded: bool,
}

#[tauri::command]
pub async fn list_models() -> Result<Vec<ModelEntry>, String> {
    let downloaded = altgo::model::list_downloaded();
    let entries = altgo::model::models_info()
        .iter()
        .map(|m| ModelEntry {
            name: m.name.to_string(),
            filename: m.filename.to_string(),
            size_bytes: m.size_bytes,
            description: m.description.to_string(),
            downloaded: downloaded.iter().any(|d| d == m.name),
        })
        .collect();
    Ok(entries)
}

#[tauri::command]
pub async fn download_model(
    app: tauri::AppHandle,
    name: String,
) -> Result<String, String> {
    altgo::model::download_with_progress(&name, {
        let app = app.clone();
        let name_clone = name.clone();
        move |downloaded, total| {
            let _ = app.emit("model-download-progress", serde_json::json!({
                "name": name_clone,
                "downloaded": downloaded,
                "total": total,
            }));
        }
    })
    .await
    .map(|p| p.to_string_lossy().to_string())
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_model(name: String) -> Result<(), String> {
    let info = altgo::model::models_info()
        .iter()
        .find(|m| m.name == name)
        .ok_or_else(|| format!("unknown model: {}", name))?;
    let path = altgo::model::models_dir().join(info.filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn resolve_model(model: String) -> Result<Option<String>, String> {
    Ok(altgo::model::resolve_model_path(&model)
        .map(|p| p.to_string_lossy().to_string()))
}
