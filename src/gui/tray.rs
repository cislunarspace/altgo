//! System tray integration via tao 0.16 (Linux only).
//!
//! NOTE: tao's system tray (using libappindicator) is only available on Linux.

use tao::system_tray::SystemTrayBuilder;
use tao::AppHandle;

/// Build the system tray with icon and context menu (Linux only).
#[cfg(target_os = "linux")]
pub fn build_tray(app_handle: &AppHandle) -> tao::system_tray::SystemTray {
    let show_item = tao::menu::MenuItem::new("显示窗口", true, None::<&str>);
    let quit_item = tao::menu::MenuItem::new("退出", true, None::<&str>);

    let menu = tao::menu::Menu::new(&[&show_item, &quit_item]);

    // Generate a simple 32x32 microphone icon as RGBA pixels encoded as PNG.
    let icon = generate_mic_icon();

    let app = app_handle.clone();

    SystemTrayBuilder::new(icon, menu)
        .tooltip("altgo — 按住 Alt 说话")
        .on_tray_event(move |_tray, event| {
            use tao::system_tray::TrayEvent;
            match event {
                TrayEvent::Click { .. } => {
                    if let Some(window) = app.get_window("altgo") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                _ => {}
            }
        })
        .on_menu_event(|_tray, event| {
            if event.id == "quit" {
                _tray.app_handle().exit();
            }
        })
        .build()
}

/// No-op on non-Linux platforms.
#[cfg(not(target_os = "linux"))]
pub fn build_tray(_app_handle: &tao::AppHandle, _state: Arc<SharedState>) {
    // System tray is only available on Linux via libappindicator.
}

/// Generate a minimal 32x32 microphone icon as a PNG in memory.
fn generate_mic_icon() -> tao::image::Image<'static> {
    let size = 32usize;

    let mut rgba = vec![0u8; size * size * 4];

    // Draw microphone body (light blue filled circle).
    let cx = size / 2;
    let cy = size / 2 - 2;
    let radius = 7i32;

    for dy in -(radius + 2)..=(radius + 2) {
        for dx in -(radius + 2)..=(radius + 2) {
            let dist_sq = dx * dx + dy * dy;
            if dist_sq <= radius * radius {
                let px = (cx as i32 + dx) as usize;
                let py = (cy as i32 + dy) as usize;
                if px < size && py < size {
                    let idx = (py * size + px) * 4;
                    rgba[idx] = 74; // R — #4A9FFF
                    rgba[idx + 1] = 159; // G
                    rgba[idx + 2] = 255; // B
                    rgba[idx + 3] = 255; // A
                }
            }
        }
    }

    // Draw mic stand (small rounded rect at bottom).
    let stand_top = cy + radius - 2;
    let stand_bottom = stand_top + 8;
    let stand_left = cx - 3;
    let stand_right = cx + 3;

    for y in stand_top..stand_bottom {
        for x in stand_left..=stand_right {
            if x < size && y < size {
                let idx = (y * size + x) * 4;
                rgba[idx] = 74;
                rgba[idx + 1] = 159;
                rgba[idx + 2] = 255;
                rgba[idx + 3] = 255;
            }
        }
    }

    // Encode as PNG.
    let mut png_data = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png_data, size as u32, size as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&rgba).unwrap();
    }

    tao::image::Image::from_bytes(&png_data).unwrap()
}
