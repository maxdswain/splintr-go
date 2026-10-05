# Development

Requires Go 1.26+, Rust/Cargo, Make and a C compiler. Windows uses MinGW-w64 under MSYS2. Install `jq` for license collection and `tar` for packaging; no Python is needed.

```sh
make build                # Rust library and Go package
make test                 # Rust tests, go vet, Go race tests
make fmt                  # rustfmt and Go formatters
make fmt-check lint       # rustfmt, Clippy and Revive (golangci-lint v2)
make licenses             # dependency notices in build/licenses
make install PREFIX="$HOME/.local"
make test-release VERSION=v0.1.0
```

`make install` puts the archive and native linker flags in `PREFIX/lib`, the header in `PREFIX/include`, and notices in `PREFIX/share/licenses/splintr-go`. `DESTDIR` supports staged installation. See [consumer linker setup](releases.md).

Go uses Revive with a 120-column limit; rustfmt uses `max_width = 120`. CI runs both format checks and Clippy.

## Features

All bundled vocabularies are enabled by default. For JSON-only or selected-vocabulary builds:

```sh
make test NO_DEFAULT_FEATURES=1
make build NO_DEFAULT_FEATURES=1 FEATURES=splintr/vocab-qwen
```

Use the same feature settings when building, testing and installing. Releases always bundle every vocabulary. Full bundles increase binary size and carry additional license terms.

Set `GOOS`, `GOARCH` and `TARGET` for cross-compilation, with matching Rust and C toolchains. For example, Linux arm64 uses `TARGET=aarch64-unknown-linux-gnu`.

## Layout

| Path | Purpose |
| --- | --- |
| `splintr.go` | Public Go package |
| `tests/`, `tests/test_data/` | Go integration tests and fixtures |
| `native/src/lib.rs`, `native/tests/unit.rs` | Rust C ABI and unit tests |
| `native/include/splintr.h` | C interface and ownership rules |
| `Makefile`, `.github/workflows/` | Build, test and release tasks |
| `build/`, `native/target/` | Ignored build output |

Native linker flags come from rustc, not a hardcoded platform list. Go builds use `-a` because its cache does not track static-archive changes. `test-release` runs the integration tests from a separate consumer module against an extracted archive.
