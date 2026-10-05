# Releases

Download the archive and `.sha256` file matching your module version and platform from [Releases](https://github.com/maxdswain/splintr-go/releases). Prebuilt libraries need Go 1.26+ and a C compiler, not Rust.

| Platform | Build baseline |
| --- | --- |
| Linux amd64 / arm64 | Ubuntu 24.04, glibc |
| macOS amd64 / arm64 | macOS 15 |
| Windows amd64 | MinGW-w64; MSVC is unsupported |

## Linux / macOS

From your application module, after downloading both files:

```sh
version=v0.1.0
archive="splintr-go-$version-$(go env GOOS)-$(go env GOARCH).tar.gz"
sha256sum -c "$archive.sha256" # macOS: shasum -a 256 -c "$archive.sha256"
mkdir -p splintr-native
tar -xzf "$archive" -C splintr-native

go get "github.com/maxdswain/splintr-go@$version"
SPLINTR_LIB="$PWD/splintr-native/lib"
export CGO_LDFLAGS="\"-L$SPLINTR_LIB\" -lsplintr_go $(cat "$SPLINTR_LIB/native-static-libs.txt")"
CGO_ENABLED=1 go build -a ./...
```

Stop if verification fails. Checksums detect corruption, not publisher identity; only extract trusted archives into a fresh directory.

## Windows PowerShell

Put MinGW-w64 GCC on `PATH`, download the Windows archive and checksum, then:

```powershell
$version = 'v0.1.0'
$archive = "splintr-go-$version-windows-amd64.tar.gz"
$expected = (Get-Content "$archive.sha256").Split(' ')[0]
if ((Get-FileHash $archive -Algorithm SHA256).Hash -ne $expected) { throw 'Checksum mismatch' }
New-Item -ItemType Directory -Force splintr-native | Out-Null
tar -xzf $archive -C splintr-native

go get "github.com/maxdswain/splintr-go@$version"
$lib = (Resolve-Path './splintr-native/lib').Path.Replace('\', '/')
$flags = (Get-Content "$lib/native-static-libs.txt" -Raw).Trim()
$env:CGO_LDFLAGS = "`"-L$lib`" -lsplintr_go $flags"
$env:CGO_ENABLED = '1'
go build -a ./...
```

Keep these environment settings for subsequent builds. The supplied flags preserve Rust's native dependency order. Use `-a` after replacing the library.

## Build and publish

```sh
make release VERSION=v0.1.0       # build/release/*.tar.gz and checksums
make test-release VERSION=v0.1.0  # rebuild, extract, run isolated consumer tests
```

Archives contain `lib/libsplintr_go.a`, `lib/native-static-libs.txt`, `include/splintr.h`, and `share/licenses/splintr-go/`. All bundled vocabularies are included. Explicit cross-target builds require matching `GOOS`, `GOARCH`, `TARGET` and installed toolchains.

Pushes to `main` automatically release qualifying Conventional Commits using [git-cliff](https://git-cliff.org/):

- The first release is `v0.1.0`.
- `feat:` bumps the minor version; `fix:` and `perf:` bump the patch version.
- `!` or a `BREAKING CHANGE:` footer bumps the major version, including from `0.x` to `1.0.0`.
- Other commits do not trigger a release unless they declare a breaking change.

The workflow generates release notes, tests all five platforms, and verifies archive checksums before creating the tag.
It uploads the assets to a draft, then publishes it once every upload succeeds. A failed build creates no tag or release.
Release runs are serialized to avoid competing version bumps. Rules and changelog formatting live in `cliff.toml`.

Manually pushed `v*` tags also build and publish. Manual workflow dispatch builds artifacts only.
Tags created by the workflow use `GITHUB_TOKEN`, so they do not trigger a second workflow; publishing runs in the same job.
Review vocabulary license changes before merging to `main`, and preserve the license bundle when redistributing.
