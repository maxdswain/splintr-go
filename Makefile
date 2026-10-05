SHELL := /bin/sh
ROOT := $(abspath $(dir $(lastword $(MAKEFILE_LIST))))
MANIFEST := $(ROOT)/native/Cargo.toml
NATIVE := $(ROOT)/build/native
LICENSES := $(ROOT)/build/licenses
RELEASES := $(ROOT)/build/release
PREFIX ?= /usr/local
TARGET ?=
FEATURES ?=
NO_DEFAULT_FEATURES ?= 0
GOOS ?= $(shell go env GOOS)
GOARCH ?= $(shell go env GOARCH)

RUST_linux_amd64 := x86_64-unknown-linux-gnu
RUST_linux_arm64 := aarch64-unknown-linux-gnu
RUST_darwin_amd64 := x86_64-apple-darwin
RUST_darwin_arm64 := aarch64-apple-darwin
RUST_windows_amd64 := x86_64-pc-windows-gnu
EXPECTED_TARGET = $(RUST_$(GOOS)_$(GOARCH))
RELEASE_TARGET = $(if $(TARGET),$(TARGET),$(EXPECTED_TARGET))
ARCHIVE = splintr-go-$(VERSION)-$(GOOS)-$(GOARCH).tar.gz
RUST_ARCHIVE = $(ROOT)/native/target/$(if $(TARGET),$(TARGET)/)release/libsplintr_go.a
CARGO_FLAGS = --locked --manifest-path "$(MANIFEST)" $(if $(TARGET),--target "$(TARGET)") \
	$(if $(FEATURES),--features "$(FEATURES)") $(if $(filter 1,$(NO_DEFAULT_FEATURES)),--no-default-features)
GO_TAGS = $(if $(filter 1,$(NO_DEFAULT_FEATURES)),,-tags=bundled)
GO_ENV = CGO_ENABLED=1 CGO_LDFLAGS="$${CGO_LDFLAGS:+$$CGO_LDFLAGS }$$(cat "$(NATIVE)/cgo-ldflags.txt")"
CHECKSUM = $(shell command -v sha256sum >/dev/null && echo sha256sum || echo 'shasum -a 256')

.PHONY: all build native test fmt fmt-check lint licenses install check-version check-release release test-release clean
all: build

# External archive changes are not tracked by Go's cache; force rebuilding.
build: native
	$(GO_ENV) go build -a ./...

native:
	@set -eu; mkdir -p "$(NATIVE)"; \
	CARGO_TERM_COLOR=never cargo rustc --release $(CARGO_FLAGS) -- --print=native-static-libs \
		>"$(NATIVE)/rustc.log" 2>&1 || { cat "$(NATIVE)/rustc.log"; exit 1; }; \
	cat "$(NATIVE)/rustc.log"; \
	sed -n 's/^note: native-static-libs: //p' "$(NATIVE)/rustc.log" | tail -1 >"$(NATIVE)/native-static-libs.txt"; \
	test -s "$(NATIVE)/native-static-libs.txt"; \
	cp "$(RUST_ARCHIVE)" "$(NATIVE)/libsplintr_go.a"; \
	libdir="$(NATIVE)"; \
	if command -v cygpath >/dev/null; then libdir=$$(cygpath -m "$$libdir"); fi; \
	libdir=$$(printf '%s' "$$libdir" | sed 's/\\/\\\\/g; s/"/\\"/g'); \
	printf '"-L%s" -lsplintr_go %s\n' "$$libdir" "$$(cat "$(NATIVE)/native-static-libs.txt")" >"$(NATIVE)/cgo-ldflags.txt"

test: native
	cargo test $(CARGO_FLAGS)
	$(GO_ENV) go vet -a $(GO_TAGS) ./...
	$(GO_ENV) go test -race -a $(GO_TAGS) ./...

fmt:
	cargo fmt --manifest-path "$(MANIFEST)"
	golangci-lint fmt

fmt-check:
	cargo fmt --manifest-path "$(MANIFEST)" -- --check
	golangci-lint fmt --diff

lint: native
	cargo clippy --locked --manifest-path "$(MANIFEST)" --all-targets --all-features -- -D warnings
	$(GO_ENV) golangci-lint run

# Copy dependency terms and provenance, not vocabulary payloads or source code.
licenses:
	@set -eu; mkdir -p "$(ROOT)/build"; \
	cargo metadata $(subst --target,--filter-platform,$(CARGO_FLAGS)) --format-version 1 >"$(ROOT)/build/dependencies.json"; \
	rm -rf "$(LICENSES)"; mkdir -p "$(LICENSES)/packages"; \
	cp "$(ROOT)/LICENSE" "$(ROOT)/NOTICE" "$(ROOT)/LICENSE-OTHERS" "$(LICENSES)/"; \
	jq -c ". as \$$m | .packages[] \
		| select(.id as \$$id | \$$m.resolve.nodes | any(.id == \$$id)) \
		| select(.id as \$$id | \$$m.workspace_members | index(\$$id) | not) \
		| {name, version, license, license_file, repository, source, manifest_path}" \
		"$(ROOT)/build/dependencies.json" >"$(LICENSES)/packages.jsonl"; \
	while IFS= read -r package; do \
		dest="$(LICENSES)/packages/$$(printf '%s' "$$package" | jq -r '.name + "/" + .version')"; \
		source=$$(printf '%s' "$$package" | jq -r '.manifest_path'); \
		if command -v cygpath >/dev/null; then source=$$(cygpath -u "$$source"); fi; \
		source=$$(dirname "$$source"); mkdir -p "$$dest"; \
		printf '%s\n' "$$package" | jq . >"$$dest/metadata.json"; \
		find "$$source" -type f \( -iname 'LICENSE*' -o -iname 'LICENCE*' \
			-o -iname 'NOTICE*' -o -iname 'COPYING*' -o -iname 'COPYRIGHT*' \) >"$$dest/files"; \
		declared=$$(printf '%s' "$$package" | jq -r '.license_file // empty'); \
		if test -n "$$declared"; then \
			case "$$declared" in \
				[A-Za-z]:*) declared=$$(cygpath -u "$$declared");; \
				/*) ;; \
				*) declared="$$source/$$declared";; \
			esac; \
			test -s "$$declared"; cp "$$declared" "$$dest/DECLARED-LICENSE"; \
		else \
			test -s "$$dest/files" || { echo "Missing license: $$source" >&2; exit 1; }; \
		fi; \
		while IFS= read -r file; do \
			rel=$${file#"$$source"/}; mkdir -p "$$dest/$$(dirname "$$rel")"; cp "$$file" "$$dest/$$rel"; \
		done <"$$dest/files"; rm "$$dest/files"; \
		for file in "$$source"/README* "$$source"/AUTHOR*; do \
			if test -f "$$file"; then cp "$$file" "$$dest/"; fi; \
		done; \
	done <"$(LICENSES)/packages.jsonl"

install: native licenses
	mkdir -p "$(DESTDIR)$(PREFIX)/lib" "$(DESTDIR)$(PREFIX)/include" "$(DESTDIR)$(PREFIX)/share/licenses"
	install -m 0644 "$(NATIVE)/libsplintr_go.a" "$(NATIVE)/native-static-libs.txt" "$(DESTDIR)$(PREFIX)/lib/"
	install -m 0644 "$(ROOT)/native/include/splintr.h" "$(DESTDIR)$(PREFIX)/include/"
	rm -rf "$(DESTDIR)$(PREFIX)/share/licenses/splintr-go"
	cp -R "$(LICENSES)" "$(DESTDIR)$(PREFIX)/share/licenses/splintr-go"

check-version:
	@number='(0|[1-9][0-9]*)'; identifier="($$number|[0-9]*[A-Za-z-][0-9A-Za-z-]*)"; \
	printf '%s\n' "$(VERSION)" | grep -Eq "^v$$number\.$$number\.$$number(-$$identifier(\.$$identifier)*)?$$" \
		|| { echo 'VERSION must be vX.Y.Z with an optional SemVer prerelease' >&2; exit 1; }

check-release: check-version
	@test "$(NO_DEFAULT_FEATURES)" != 1 || { echo 'Releases require bundled vocabularies' >&2; exit 1; }
	@test -n "$(EXPECTED_TARGET)" && test "$(RELEASE_TARGET)" = "$(EXPECTED_TARGET)" \
		|| { echo 'Unsupported platform or mismatched TARGET' >&2; exit 1; }

release: check-release
	$(MAKE) native licenses TARGET="$(RELEASE_TARGET)" FEATURES=vocabs NO_DEFAULT_FEATURES=0
	@set -eu; stage="$(RELEASES)/stage"; rm -rf "$$stage"; \
	mkdir -p "$$stage/lib" "$$stage/include" "$$stage/share/licenses"; \
	cp "$(NATIVE)/libsplintr_go.a" "$(NATIVE)/native-static-libs.txt" "$$stage/lib/"; \
	cp "$(ROOT)/native/include/splintr.h" "$$stage/include/"; \
	cp -R "$(LICENSES)" "$$stage/share/licenses/splintr-go"; \
	tar -czf "$(RELEASES)/$(ARCHIVE).tmp" -C "$$stage" lib include share; \
	mv "$(RELEASES)/$(ARCHIVE).tmp" "$(RELEASES)/$(ARCHIVE)"; rm -rf "$$stage"; \
	cd "$(RELEASES)"; $(CHECKSUM) "$(ARCHIVE)" >"$(ARCHIVE).sha256"

# Reuse the integration tests in a separate module with no local native build.
test-release: release
	@set -eu; cd "$(RELEASES)"; $(CHECKSUM) -c "$(ARCHIVE).sha256"; \
	tmp=$$(mktemp -d); trap 'rm -rf "$$tmp"' EXIT HUP INT TERM; \
	mkdir -p "$$tmp/native library" "$$tmp/module/native" "$$tmp/consumer"; \
	tar -xzf "$(ARCHIVE)" -C "$$tmp/native library"; \
	cp "$(ROOT)/go.mod" "$(ROOT)/splintr.go" "$$tmp/module/"; \
	cp -R "$(ROOT)/native/include" "$$tmp/module/native/"; \
	cp -R "$(ROOT)/tests/." "$$tmp/consumer/"; \
	printf 'module release-test\n\ngo 1.26.0\n\nrequire github.com/maxdswain/splintr-go v0.0.0\nreplace github.com/maxdswain/splintr-go => ../module\n' >"$$tmp/consumer/go.mod"; \
	flags=$$(cat "$$tmp/native library/lib/native-static-libs.txt"); libdir="$$tmp/native library/lib"; \
	if command -v cygpath >/dev/null; then libdir=$$(cygpath -m "$$libdir"); fi; \
	cd "$$tmp/consumer"; CGO_ENABLED=1 CGO_LDFLAGS="\"-L$$libdir\" -lsplintr_go $$flags" go test -race -a -tags=bundled ./...

clean:
	cargo clean --manifest-path "$(MANIFEST)"
	rm -rf "$(ROOT)/build"
