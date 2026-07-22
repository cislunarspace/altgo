//! 保证 `tauri.conf.json` 里 `../target/deps/bin/*` 在未下载依赖时仍能匹配到文件，避免 `glob pattern ... didn't match any files`。
//! 真实二进制由 `packaging/scripts/download-deps.ps1`（Windows）或 `make deps-linux` 等放入同一目录；存在任意非占位文件时会删除占位项。

use std::path::{Path, PathBuf};

const PLACEHOLDER: &str = "00_tauri_deps_placeholder.txt";

fn main() {
    sync_deps_bin_placeholder();

    // exe 和 lib 测试二进制都链接 rfd（tauri-plugin-dialog 的依赖），其静态
    // 导入 TaskDialogIndirect 只有 comctl32 v6 才导出。tauri-build 默认以
    // 资源形式给 exe 嵌入 manifest，测试二进制没有，进程启动即
    // STATUS_ENTRYPOINT_NOT_FOUND（0xc0000139），Windows 上整个测试套件
    // 无法运行。改为关掉 tauri-build 的 manifest 资源嵌入（避免与链接参数
    // 产生重复资源 CVT1100），统一用 /MANIFESTINPUT 链接参数注入同一份
    // manifest，exe 与测试二进制同时覆盖。
    let attrs = tauri_build::Attributes::new()
        .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
    tauri_build::try_build(attrs).expect("failed to run tauri-build");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let manifest = PathBuf::from(
            std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be set"),
        )
        .join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    }
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
