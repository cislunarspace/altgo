.PHONY: build test install clean fmt lint deps-linux deps-windows package-deb package-msi

BINARY=altgo

build:
	cargo build --release
	cp target/release/$(BINARY) $(BINARY)

test:
	cargo test

fmt:
	cargo fmt -- --check

lint:
	cargo clippy -- -D warnings

install: build
	install -d $(DESTDIR)/usr/local/bin
	install -m 755 $(BINARY) $(DESTDIR)/usr/local/bin/$(BINARY)
	install -d $(DESTDIR)/etc/altgo
	install -m 644 configs/altgo.toml $(DESTDIR)/etc/altgo/altgo.toml

clean:
	cargo clean
	rm -f $(BINARY)

# Download dependencies for packaging
deps-linux:
	bash scripts/download-deps.sh

deps-windows:
	pwsh scripts/download-deps.ps1

# Build deb package (Linux, run on Linux)
package-deb: deps-linux build
	cargo deb --no-build

# Build MSI package (Windows, run on Windows)
package-msi: deps-windows build
	pwsh -Command "wix build msi/Product.wxs -d Version=$$(cargo metadata --format-version 1 | jq -r '.packages[0].version') -d SourceDir=target/release -o target/altgo-x86_64-windows.msi"
