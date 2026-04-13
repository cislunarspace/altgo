.PHONY: build test install clean fmt lint

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
