//! System tray integration via tao 0.16 (Linux and Windows).
//!
//! Uses the correct tao 0.16.11 API.

use tao::{
    event_loop::EventLoopWindowTarget,
    menu::{ContextMenu, MenuItemAttributes},
    system_tray::SystemTrayBuilder,
    AppHandle, Icon,
};

use super::i18n;

/// Generate a minimal 32x32 microphone icon as RGBA pixels.
fn generate_mic_icon() -> Icon {
    let size = 32usize;

    let mut rgba = vec![0u8; size * size * 4];

    // Draw microphone body (light blue filled circle).
    let cx = size / 2;
    let cy = size / 2 - 2;
    let radius: i32 = 7;

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
    let stand_top = cy as usize + radius as usize - 2;
    let stand_bottom = stand_top + 8;
    let stand_left = cx - 3;
    let stand_right = cx + 3;

    for y in stand_top..stand_bottom {
        for x in stand_left..=stand_right {
            let x = x as usize;
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

    Icon::from_bytes(&png_data).expect("failed to create icon from PNG")
}

/// Build the system tray with icon and context menu.
#[cfg(any(target_os = "linux", target_os = "windows"))]
pub fn build_tray(
    _app_handle: &AppHandle,
    window_target: &EventLoopWindowTarget<()>,
    lang: i18n::Lang,
) -> tao::system_tray::SystemTray {
    let icon = generate_mic_icon();

    let mut menu = ContextMenu::new();
    menu.add_item(MenuItemAttributes::new(i18n::t("tray.show", lang)));
    menu.add_item(MenuItemAttributes::new(i18n::t("tray.settings", lang)));
    menu.add_item(MenuItemAttributes::new(i18n::t("tray.exit", lang)));

    SystemTrayBuilder::new(icon, Some(menu))
        .with_tooltip(i18n::t("tray.tooltip", lang))
        .build(window_target)
        .expect("failed to build system tray")
}

/// No-op on non-Linux/Windows platforms.
#[cfg(not(any(target_os = "linux", target_os = "windows")))]
#[allow(unused_variables)]
pub fn build_tray(
    app_handle: &AppHandle,
    window_target: &EventLoopWindowTarget<()>,
    lang: i18n::Lang,
) -> tao::system_tray::SystemTray {
    unimplemented!("System tray not supported on this platform")
}
