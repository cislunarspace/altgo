//! Tauri commands — 前端通过 IPC 调用的函数。

use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Deserializer, Serialize};
use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, Monitor, PhysicalPosition, PhysicalSize, State,
};

use crate::{
    config, history, key_listener, output, pipeline, polisher, recorder, state_machine,
    transcriber, AppState,
};

const OVERLAY_RECORDING_W: f64 = 200.0;
const OVERLAY_RECORDING_H: f64 = 48.0;
const OVERLAY_RESULT_W: f64 = 520.0;
const OVERLAY_RESULT_H: f64 = 100.0;
const OVERLAY_BOTTOM_OFFSET: f64 = 80.0;

#[cfg(target_os = "windows")]
#[repr(C)]
struct WinPoint {
    x: i32,
    y: i32,
}

#[cfg(target_os = "windows")]
#[link(name = "user32")]
extern "system" {
    fn GetAsyncKeyState(vKey: i32) -> i16;
    fn GetCursorPos(lpPoint: *mut WinPoint) -> i32;
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

fn resolve_vk_for_pipeline(cfg: &config::KeyListenerConfig) -> Result<i32, String> {
    if let Some(vk) = cfg.windows_vk {
        return Ok(vk);
    }
    resolve_vk_code(&cfg.key_name)
}

fn apply_nested_opt_u16(target: &mut Option<u16>, patch: Option<Option<u16>>) {
    match patch {
        None => {}
        Some(None) => *target = None,
        Some(Some(v)) => *target = Some(v),
    }
}

fn apply_nested_opt_i32(target: &mut Option<i32>, patch: Option<Option<i32>>) {
    match patch {
        None => {}
        Some(None) => *target = None,
        Some(Some(v)) => *target = Some(v),
    }
}

/// 区分 JSON 中「字段缺失」与「`null`」：前者不修改配置，后者清除已保存的 evdev 键码。
fn deserialize_opt_patch_u16<'de, D>(deserializer: D) -> Result<Option<Option<u16>>, D::Error>
where
    D: Deserializer<'de>,
{
    let v = serde_json::Value::deserialize(deserializer)?;
    match v {
        serde_json::Value::Null => Ok(Some(None)),
        serde_json::Value::Number(n) => {
            let u = n
                .as_u64()
                .ok_or_else(|| serde::de::Error::custom("linux_evdev_code: expected number"))?;
            if u > u16::MAX as u64 {
                return Err(serde::de::Error::custom("linux_evdev_code out of range"));
            }
            Ok(Some(Some(u as u16)))
        }
        _ => Err(serde::de::Error::custom(
            "linux_evdev_code: expected null or number",
        )),
    }
}

/// 同上，用于 Windows 虚拟键码。
fn deserialize_opt_patch_i32<'de, D>(deserializer: D) -> Result<Option<Option<i32>>, D::Error>
where
    D: Deserializer<'de>,
{
    let v = serde_json::Value::deserialize(deserializer)?;
    match v {
        serde_json::Value::Null => Ok(Some(None)),
        serde_json::Value::Number(n) => {
            let i = n
                .as_i64()
                .ok_or_else(|| serde::de::Error::custom("windows_vk: expected integer"))?;
            if i < i32::MIN as i64 || i > i32::MAX as i64 {
                return Err(serde::de::Error::custom("windows_vk out of range"));
            }
            Ok(Some(Some(i as i32)))
        }
        _ => Err(serde::de::Error::custom(
            "windows_vk: expected null or number",
        )),
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

/// Linux: root-window mouse coordinates from `xdotool` (physical pixels).
#[cfg(target_os = "linux")]
fn mouse_position_physical() -> Option<(i32, i32)> {
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

    tracing::debug!("mouse position (physical): ({}, {})", mouse_x, mouse_y);
    Some((mouse_x, mouse_y))
}

#[cfg(target_os = "windows")]
fn mouse_position_physical() -> Option<(i32, i32)> {
    let mut pt = WinPoint { x: 0, y: 0 };
    // SAFETY: GetCursorPos writes to a valid stack POINT; standard Win32 API.
    let ok = unsafe { GetCursorPos(&mut pt) };
    if ok == 0 {
        return None;
    }
    Some((pt.x, pt.y))
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
fn mouse_position_physical() -> Option<(i32, i32)> {
    None
}

/// Pick the monitor under the cursor using the same coordinate space as Tauri/GTK (`monitor_from_point`).
/// Falls back to the primary monitor if the cursor cannot be read or no monitor contains the point.
fn resolve_monitor_at_cursor(overlay: &tauri::WebviewWindow) -> Option<Monitor> {
    if let Some((x, y)) = mouse_position_physical() {
        match overlay.monitor_from_point(x as f64, y as f64) {
            Ok(Some(m)) => return Some(m),
            Ok(None) => tracing::debug!("monitor_from_point: no monitor for cursor ({}, {})", x, y),
            Err(e) => tracing::warn!(error = %e, "monitor_from_point failed"),
        }
    } else {
        tracing::debug!("mouse position unavailable; using primary monitor for overlay");
    }

    overlay.primary_monitor().ok().flatten()
}

fn position_overlay(overlay: &tauri::WebviewWindow, width: f64, height: f64) {
    let _ = overlay.set_size(LogicalSize::new(width, height));

    let Some(monitor) = resolve_monitor_at_cursor(overlay) else {
        tracing::warn!("no monitor available for overlay positioning");
        return;
    };

    let scale = monitor.scale_factor();
    let phys: PhysicalSize<u32> = LogicalSize::new(width, height).to_physical(scale);
    let phys_w = phys.width as i32;
    let phys_h = phys.height as i32;
    let offset_phys = (OVERLAY_BOTTOM_OFFSET * scale).round() as i32;

    let pos = monitor.position();
    let size = monitor.size();
    let mw = size.width as i32;
    let mh = size.height as i32;

    let x = pos.x + (mw - phys_w) / 2;
    let y = pos.y + mh - phys_h - offset_phys;

    let _ = overlay.set_position(PhysicalPosition::new(x, y));
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigResponse {
    pub key_name: String,
    pub linux_evdev_code: Option<u16>,
    pub windows_vk: Option<i32>,
    pub language: String,
    pub engine: String,
    pub model: String,
    pub api_base_url: String,
    pub polish_level: String,
    pub polish_model: String,
    pub polish_api_base_url: String,
    pub gui_language: String,
    pub transcriber_api_key: String,
    pub polisher_api_key: String,
}

#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<ConfigResponse, String> {
    let cfg = state.config.lock().await;
    Ok(ConfigResponse {
        key_name: cfg.key_listener.key_name.clone(),
        linux_evdev_code: cfg.key_listener.linux_evdev_code,
        windows_vk: cfg.key_listener.windows_vk,
        language: cfg.transcriber.language.clone(),
        engine: cfg.transcriber.engine.clone(),
        model: cfg.transcriber.model.clone(),
        api_base_url: cfg.transcriber.api_base_url.clone(),
        polish_level: cfg.polisher.level.clone(),
        polish_model: cfg.polisher.model.clone(),
        polish_api_base_url: cfg.polisher.api_base_url.clone(),
        gui_language: cfg.gui.language.clone(),
        transcriber_api_key: cfg.transcriber.api_key.clone(),
        polisher_api_key: cfg.polisher.api_key.clone(),
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveConfigRequest {
    pub key_name: Option<String>,
    /// `None` = 请求中未带此字段（不修改）；`Some(None)` = JSON `null`（清除）；`Some(Some)` = 设置键码。
    #[serde(default, deserialize_with = "deserialize_opt_patch_u16")]
    pub linux_evdev_code: Option<Option<u16>>,
    #[serde(default, deserialize_with = "deserialize_opt_patch_i32")]
    pub windows_vk: Option<Option<i32>>,
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

/// Start the main voice pipeline thread (used at app startup).
pub fn spawn_pipeline_at_startup(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    let cfg_arc = Arc::new(state.config.blocking_lock().clone());
    cfg_arc.validate().map_err(|e| e.to_string())?;
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    let app_handle = app.clone();
    let pipeline_status = state.pipeline_status.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to build tokio runtime");
        rt.block_on(run_pipeline(app_handle, cfg_arc, stop_rx, pipeline_status));
    });
    *state.pipeline.blocking_lock() = Some(crate::PipelineHandle { stop_tx });
    Ok(())
}

/// Stop the current pipeline and start a new one using the latest in-memory config (after save).
async fn restart_pipeline(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    {
        let mut p = state.pipeline.lock().await;
        if let Some(h) = p.take() {
            let _ = h.stop_tx.send(());
        }
    }
    tokio::time::sleep(Duration::from_millis(320)).await;
    let cfg_arc = Arc::new(state.config.lock().await.clone());
    cfg_arc.validate().map_err(|e| e.to_string())?;
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    let app_handle = app.clone();
    let pipeline_status = state.pipeline_status.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to build tokio runtime");
        rt.block_on(run_pipeline(app_handle, cfg_arc, stop_rx, pipeline_status));
    });
    *state.pipeline.lock().await = Some(crate::PipelineHandle { stop_tx });
    Ok(())
}

#[tauri::command]
pub async fn save_config(
    app: AppHandle,
    state: State<'_, AppState>,
    req: SaveConfigRequest,
) -> Result<(), String> {
    let mut cfg = state.config.lock().await;

    if let Some(v) = req.key_name {
        cfg.key_listener.key_name = v;
    }
    apply_nested_opt_u16(&mut cfg.key_listener.linux_evdev_code, req.linux_evdev_code);
    apply_nested_opt_i32(&mut cfg.key_listener.windows_vk, req.windows_vk);
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

    cfg.validate().map_err(|e| e.to_string())?;
    cfg.save(&state.config_path)
        .map_err(|e| format!("save failed: {}", e))?;
    drop(cfg);

    restart_pipeline(app).await
}

#[tauri::command]
pub async fn capture_activation_key(
    state: State<'_, AppState>,
) -> Result<crate::key_capture::CaptureActivationResponse, String> {
    {
        let mut p = state.pipeline.lock().await;
        if let Some(h) = p.take() {
            let _ = h.stop_tx.send(());
        }
    }
    tokio::time::sleep(Duration::from_millis(400)).await;

    match tokio::task::spawn_blocking(crate::key_capture::capture_activation_key_blocking).await {
        Ok(Ok(r)) => Ok(r),
        Ok(Err(e)) => Err(e),
        Err(e) => Err(e.to_string()),
    }
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
    let vk_code = match resolve_vk_for_pipeline(&cfg.key_listener) {
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

    // Validate recorder / transcriber / polisher before starting global key capture so errors
    // are visible immediately and we do not leave a key listener running without a main loop.
    let mut recorder =
        recorder::PlatformRecorder::new(cfg.recorder.sample_rate, cfg.recorder.channels);

    let model_path = if cfg.transcriber.engine == "local" {
        match crate::model::resolve_model_path(&cfg.transcriber.model) {
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

    let polish_level = polisher::PolishLevel::from_str(&cfg.polisher.level).unwrap_or_else(|_| {
        tracing::warn!("invalid polish level, using medium");
        polisher::PolishLevel::Medium
    });
    let polisher_protocol =
        polisher::ApiProtocol::from_str(&cfg.polisher.protocol).unwrap_or_else(|e| {
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

    // Must keep `PlatformListener` alive for the whole pipeline: its `Drop` stops xinput/evtest.
    // Previously the listener lived only inside the block below and was dropped before the main
    // loop — global keys were never delivered.
    #[cfg(not(target_os = "windows"))]
    let _linux_key_listener = {
        let mut listener = match key_listener::PlatformListener::new(&cfg.key_listener) {
            Ok(l) => l,
            Err(e) => {
                tracing::error!(error = %e, "failed to create key listener");
                let _ = app.emit("pipeline-error", format!("key listener: {}", e));
                return;
            }
        };
        let (mut key_events, key_backend) = match listener.start() {
            Ok(pair) => pair,
            Err(e) => {
                tracing::error!(error = %e, "failed to start key listener");
                let _ = app.emit("pipeline-error", format!("key listener start: {}", e));
                return;
            }
        };
        tracing::info!(backend = key_backend, "Linux key listener active");
        let _ = app.emit("key-listener-backend", key_backend);
        // Forward PlatformListener events into the shared raw_key channel.
        let poll_running = poll_running.clone();
        std::thread::spawn(move || {
            use tokio::sync::mpsc::error::TryRecvError;
            while poll_running.load(Ordering::SeqCst) {
                match key_events.try_recv() {
                    Ok(ev) => {
                        let _ = raw_key_tx.send(ev);
                    }
                    Err(TryRecvError::Empty) => {
                        std::thread::sleep(std::time::Duration::from_millis(poll_interval_ms));
                    }
                    Err(TryRecvError::Disconnected) => {
                        tracing::error!("key listener channel closed unexpectedly");
                        break;
                    }
                }
            }
        });
        listener
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
                                    // 以原始转写为准：润色侧可能返回空字符串，此时仍应展示/剪贴/入库。
                                    if !output.raw_text.is_empty() {
                                        let text_to_use = if cfg.output.prefer_polished
                                            && !output.polish_failed
                                            && !output.text.trim().is_empty()
                                        {
                                            &output.text
                                        } else {
                                            &output.raw_text
                                        };
                                        let display_text = text_to_use.clone();

                                        let hist_path =
                                            app.state::<AppState>().history_path.clone();
                                        let raw_hist = output.raw_text.clone();
                                        let text_hist = display_text.clone();
                                        match tokio::task::spawn_blocking(move || {
                                            history::append_entry(&hist_path, raw_hist, text_hist)
                                        })
                                        .await
                                        {
                                            Ok(Ok(_)) => {
                                                let _ = app.emit("history-updated", ());
                                            }
                                            Ok(Err(e)) => {
                                                tracing::warn!(
                                                    error = %e,
                                                    "failed to append transcription history"
                                                );
                                            }
                                            Err(e) => {
                                                tracing::warn!(
                                                    error = %e,
                                                    "history append task failed"
                                                );
                                            }
                                        }

                                        let _ =
                                            output::write_clipboard(&display_text).await;

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
    let downloaded = crate::model::list_downloaded();
    let entries = crate::model::models_info()
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
pub async fn download_model(app: tauri::AppHandle, name: String) -> Result<String, String> {
    if let Some(info) = crate::model::models_info()
        .iter()
        .find(|m| m.name == name.as_str())
    {
        let _ = app.emit(
            "model-download-progress",
            serde_json::json!({
                "name": name,
                "downloaded": 0_u64,
                "total": info.size_bytes,
            }),
        );
    }
    crate::model::download_with_progress(&name, {
        let app = app.clone();
        let name_clone = name.clone();
        move |downloaded, total| {
            let _ = app.emit(
                "model-download-progress",
                serde_json::json!({
                    "name": name_clone,
                    "downloaded": downloaded,
                    "total": total,
                }),
            );
        }
    })
    .await
    .map(|p| p.to_string_lossy().to_string())
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_model(name: String) -> Result<(), String> {
    let info = crate::model::models_info()
        .iter()
        .find(|m| m.name == name)
        .ok_or_else(|| format!("unknown model: {}", name))?;
    let path = crate::model::models_dir().join(info.filename);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn resolve_model(model: String) -> Result<Option<String>, String> {
    Ok(crate::model::resolve_model_path(&model).map(|p| p.to_string_lossy().to_string()))
}

#[tauri::command]
pub async fn list_history(
    state: State<'_, AppState>,
) -> Result<Vec<history::HistoryEntry>, String> {
    history::list_entries(&state.history_path).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_history_entries(
    state: State<'_, AppState>,
    ids: Vec<String>,
) -> Result<usize, String> {
    history::delete_entries(&state.history_path, &ids).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn clear_history(state: State<'_, AppState>) -> Result<(), String> {
    history::clear_all(&state.history_path).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn polish_history_entry(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<history::HistoryEntry, String> {
    let history_path = state.history_path.clone();
    let cfg = state.config.lock().await.clone();

    let entry = tokio::task::spawn_blocking({
        let p = history_path.clone();
        let id = id.clone();
        move || history::get_entry(&p, &id).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())??;

    let entry = entry.ok_or_else(|| "history entry not found".to_string())?;

    let polish_level = polisher::PolishLevel::from_str(&cfg.polisher.level).unwrap_or_else(|_| {
        tracing::warn!("invalid polish level, using medium");
        polisher::PolishLevel::Medium
    });
    let polisher_protocol =
        polisher::ApiProtocol::from_str(&cfg.polisher.protocol).unwrap_or_else(|e| {
            tracing::warn!(error = %e, "invalid polisher protocol, defaulting to openai");
            polisher::ApiProtocol::OpenAi
        });
    let formatter = polisher::LLMFormatter::with_config(
        cfg.polisher.api_key.clone(),
        cfg.polisher.api_base_url.clone(),
        cfg.polisher.model.clone(),
        cfg.polisher.timeout(),
        cfg.polisher.max_tokens,
        polisher_protocol,
        cfg.polisher.temperature,
        cfg.transcriber.language.clone(),
        cfg.polisher.system_prompt.clone(),
    )
    .map_err(|e| e.to_string())?;

    let source = entry.raw_text.clone();
    let polished = formatter
        .polish(&source, polish_level)
        .await
        .map_err(|e| e.to_string())?;

    let history_path_write = history_path;
    let out = tokio::task::spawn_blocking(move || {
        history::update_entry_text(&history_path_write, &id, polished)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    let _ = app.emit("history-updated", ());
    Ok(out)
}

#[cfg(test)]
mod save_config_request_tests {
    use super::SaveConfigRequest;

    #[test]
    fn linux_evdev_json_null_clears_stored_code() {
        let j = r#"{"linuxEvdevCode":null}"#;
        let req: SaveConfigRequest = serde_json::from_str(j).unwrap();
        assert_eq!(req.linux_evdev_code, Some(None));
    }

    #[test]
    fn linux_evdev_missing_field_means_no_patch() {
        let j = r#"{}"#;
        let req: SaveConfigRequest = serde_json::from_str(j).unwrap();
        assert!(req.linux_evdev_code.is_none());
    }

    #[test]
    fn linux_evdev_number_sets() {
        let j = r#"{"linuxEvdevCode":100}"#;
        let req: SaveConfigRequest = serde_json::from_str(j).unwrap();
        assert_eq!(req.linux_evdev_code, Some(Some(100)));
    }
}
