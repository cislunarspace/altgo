.PHONY: build test install clean fmt lint deps-linux deps-windows package-deb

BINARY=altgo

build:
	cargo build --release --manifest-path=src-tauri/Cargo.toml
	cp src-tauri/target/release/$(BINARY) $(BINARY)

test:
	cargo test --manifest-path=src-tauri/Cargo.toml

fmt:
	cargo fmt --manifest-path=src-tauri/Cargo.toml -- --check

lint:
	cargo clippy --manifest-path=src-tauri/Cargo.toml -- -D warnings

install: build
	install -d $(DESTDIR)/usr/local/bin
	install -m 755 $(BINARY) $(DESTDIR)/usr/local/bin/$(BINARY)
	install -d $(DESTDIR)/etc/altgo
	install -m 644 configs/altgo.toml $(DESTDIR)/etc/altgo/altgo.toml

clean:
	cargo clean --manifest-path=src-tauri/Cargo.toml
	rm -f $(BINARY)

deps-linux:
	bash packaging/scripts/download-deps.sh

deps-windows:
	pwsh packaging/scripts/download-deps.ps1

package-deb: deps-linux build
	cargo deb --manifest-path=src-tauri/Cargo.toml --no-build
