# Development

Requires Go 1.26+, Rust/Cargo, Make and a C compiler. Windows uses MinGW-w64 under MSYS2. Install `jq` for license collection and `tar` for packaging; no Python is needed.

```sh
make build                # Rust library and Go package
make test                 # Rust tests, go vet, Go race tests
make fmt                  # rustfmt and Go formatters
make fmt-check lint       # rustfmt, Clippy and Revive (golangci-lint v2)
make licenses             # dependency notices in build/licenses
make install PREFIX="$HOME/.local"
make test-release VERSION=v0.3.1
```

`make install` puts the archive and native linker flags in `PREFIX/lib`, the header in `PREFIX/include`, and notices in `PREFIX/share/licenses/splintr-go`. `DESTDIR` supports staged installation. See [consumer linker setup](releases.md).

Go uses Revive with a 120-column limit; rustfmt uses `max_width = 120`. CI runs both format checks and Clippy.

## Features

All bundled vocabularies are enabled by default. For JSON-only or selected-vocabulary builds:

```sh
make test NO_DEFAULT_FEATURES=1
make build NO_DEFAULT_FEATURES=1 FEATURES=splintr/vocab-qwen
```

Native batch processing uses splintr's Rayon support in both bundled and lean builds.
Splintr keeps small batches sequential and schedules larger batches on its shared Rayon thread pool.
Rebuild the native library after updating the bindings. The `v0.1.0` archive lacks batch functions,
and `v0.2.0` lacks streaming functions.

Use the same feature settings when building, testing and installing. Releases always bundle every vocabulary. Full bundles increase binary size and carry additional license terms.

Set `GOOS`, `GOARCH` and `TARGET` for cross-compilation, with matching Rust and C toolchains. For example, Linux arm64 uses `TARGET=aarch64-unknown-linux-gnu`.

## Streaming

Streaming decoding wraps splintr's owned decoder without retaining a borrowed tokenizer pointer.
A stream can outlive its tokenizer. Go serializes each stream's mutations, queries and cleanup;
copies share one cursor, while separately constructed streams are independent.

`AddToken` and `AddTokens` reject unknown IDs but keep any prefix already accepted during the call.
The lossy variants skip unknown IDs. Both emit complete UTF-8 text incrementally; an empty string
means no text was emitted. Some decoder pipelines also buffer byte-fallback runs or trailing spaces,
which are included in `HasPending` and `PendingBytes`.

`Flush` drains pending text with replacement characters for incomplete UTF-8. It neither closes nor
resets the stream; use `Reset` to discard pending text and start a fresh sequence. `Close` discards
pending text and is idempotent. Factory errors report unsupported non-incremental decoder pipelines.
Splintr 0.21.0 has no streaming encoder, so encoding remains whole-input or batch-based.

## Layout

| Path | Purpose |
| --- | --- |
| `splintr.go`, `doc.go` | Public Go API and package documentation |
| `example_test.go` | Executable examples for pkg.go.dev |
| `tests/`, `tests/test_data/` | Go integration tests and fixtures |
| `native/src/lib.rs`, `native/tests/` | Rust C ABI and unit tests |
| `native/include/splintr.h` | C interface and ownership rules |
| `Makefile`, `.github/workflows/` | Build, test and release tasks |
| `build/`, `native/target/` | Ignored build output |

Native linker flags come from rustc, not a hardcoded platform list. Go builds use `-a` because its cache does not track static-archive changes. `test-release` runs the documentation examples and integration tests against an extracted archive.
