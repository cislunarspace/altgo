//! 保证 `tauri.conf.json` 里 `../target/deps/bin/*` 在未下载依赖时仍能匹配到文件，避免 `glob pattern ... didn't match any files`。
//! 真实二进制由 `packaging/scripts/download-deps.ps1`（Windows）或 `make deps-linux` 等放入同一目录；存在任意非占位文件时会删除占位项。

use std::path::{Path, PathBuf};

const PLACEHOLDER: &str = "00_tauri_deps_placeholder.txt";

fn main() {
    sync_deps_bin_placeholder();
    tauri_build::build();
}

fn sync_deps_bin_placeholder() {
    let manifest_dir = PathBuf::from(
        std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be set"),
    );
    let deps_bin = manifest_dir.join("../target/deps/bin");
    if std::fs::create_dir_all(&deps_bin).is_err() {
        return;
    }

    let placeholder_path = deps_bin.join(PLACEHOLDER);
    let has_real = deps_bin_has_non_placeholder_files(&deps_bin);

    if has_real {
        let _ = std::fs::remove_file(&placeholder_path);
        return;
    }

    let msg = b"Placeholder so Tauri resource glob matches. Remove after running packaging/scripts/download-deps.ps1 or make deps-*.\n";
    let _ = std::fs::write(&placeholder_path, msg);
}

fn deps_bin_has_non_placeholder_files(deps_bin: &Path) -> bool {
    let Ok(rd) = std::fs::read_dir(deps_bin) else {
        return false;
    };
    for ent in rd.flatten() {
        if !ent.file_type().map(|t| t.is_file()).unwrap_or(false) {
            continue;
        }
        if ent.file_name().to_string_lossy() == PLACEHOLDER {
            continue;
        }
        return true;
    }
    false
}
