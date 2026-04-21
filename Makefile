.PHONY: build ensure-binary-deps test install clean fmt lint deps-linux deps-windows package-deb

BINARY=altgo
RELEASE_BIN_DIR=src-tauri/target/release/bin
REPO_ROOT := $(abspath .)

# 仅在缺少 ffmpeg / whisper-cli 时跑 download-deps（避免每次 make build 都 git clone）
ensure-binary-deps:
	@if [ ! -x "$(REPO_ROOT)/target/deps/bin/whisper-cli" ] || [ ! -x "$(REPO_ROOT)/target/deps/bin/ffmpeg" ]; then \
		$(MAKE) deps-linux; \
	fi

# Release 可执行文件在 src-tauri/target/release/altgo-tauri；同目录下需有 bin/whisper-cli
# （resource.rs 会查找）。因此 build 前先拉取依赖，编译后再拷贝到 target/release/bin/。
build: ensure-binary-deps
	cargo tauri build
	@install -d $(RELEASE_BIN_DIR)
	@set -e; \
	if ! ls target/deps/bin/* >/dev/null 2>&1; then \
		echo "error: target/deps/bin is empty — deps-linux failed?"; \
		exit 1; \
	fi; \
	for f in target/deps/bin/*; do \
		if [ -f "$$f" ]; then \
			install -m 755 "$$f" $(RELEASE_BIN_DIR)/; \
			echo "bundled $$(basename $$f) -> $(RELEASE_BIN_DIR)/"; \
		fi; \
	done
	@test -x $(RELEASE_BIN_DIR)/whisper-cli || ( echo "error: $(RELEASE_BIN_DIR)/whisper-cli missing — run 'make deps-linux' (needs git, cmake, g++)"; exit 1 )
	@echo "Run: src-tauri/target/release/altgo-tauri (local mode needs a GGML model from Settings)"

test:
	cargo test --manifest-path=src-tauri/Cargo.toml

fmt:
	cargo fmt --manifest-path=src-tauri/Cargo.toml -- --check

lint:
	cargo clippy --manifest-path=src-tauri/Cargo.toml -- -D warnings

install: build
	install -d $(DESTDIR)/usr/local/bin
	install -m 755 src-tauri/target/release/bundle/deb/*/usr/local/bin/$(BINARY) $(DESTDIR)/usr/local/bin/$(BINARY) 2>/dev/null || \
		install -m 755 src-tauri/target/release/altgo-tauri $(DESTDIR)/usr/local/bin/$(BINARY)
	install -d $(DESTDIR)/usr/lib/altgo/bin
	install -m 755 target/deps/bin/* $(DESTDIR)/usr/lib/altgo/bin/
	install -d $(DESTDIR)/etc/altgo
	install -m 644 configs/altgo.toml $(DESTDIR)/etc/altgo/altgo.toml

clean:
	cargo clean --manifest-path=src-tauri/Cargo.toml
	rm -f $(BINARY)

deps-linux:
	bash packaging/scripts/download-deps.sh

deps-windows:
	pwsh packaging/scripts/download-deps.ps1

package-deb: build
	cargo deb --manifest-path=src-tauri/Cargo.toml --no-build
