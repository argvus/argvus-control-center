PREFIX ?= /usr
DESTDIR ?=
CARGO ?= cargo

.PHONY: build test validate install clean

build:
	$(CARGO) build --release --workspace

test:
	$(CARGO) test --workspace

validate:
	$(CARGO) fmt --check
	$(CARGO) clippy --workspace --all-targets --all-features -- -D warnings
	$(CARGO) test --workspace
	$(CARGO) build --release --workspace
	test -x target/release/argvus-control-center
	test -f usr/share/applications/argvus-control-center.desktop
	test -f assets/argvus-about.svg

install: build
	install -Dm755 target/release/argvus-control-center $(DESTDIR)$(PREFIX)/bin/argvus-control-center
	install -Dm644 assets/argvus-about.svg $(DESTDIR)$(PREFIX)/share/argvus-control-center/argvus-about.svg
	install -Dm644 usr/share/applications/argvus-control-center.desktop $(DESTDIR)$(PREFIX)/share/applications/argvus-control-center.desktop
	install -Dm644 README.md $(DESTDIR)$(PREFIX)/share/doc/argvus-control-center/README.md
	install -Dm644 LICENSE $(DESTDIR)$(PREFIX)/share/licenses/argvus-control-center/LICENSE
clean:
	$(CARGO) clean
