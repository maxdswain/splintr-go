# splintr-go

Go bindings for [splintr](https://github.com/ml-rust/splintr) through cgo. Default builds include all 23 bundled tokenizer variants; `FromPretrained` needs no downloads.

## Installation

Requires Go 1.26+, a C compiler, `curl` and `tar`. On Linux or macOS, run these commands in your application's Go module:

```sh
go get github.com/maxdswain/splintr-go@v0.1.0
archive="splintr-go-v0.1.0-$(go env GOOS)-$(go env GOARCH).tar.gz"
curl -fL --create-dirs -o ".splintr-native/$archive" "https://github.com/maxdswain/splintr-go/releases/download/v0.1.0/$archive"
tar -xzf ".splintr-native/$archive" -C .splintr-native

CGO_ENABLED=1 CGO_LDFLAGS="\"-L$PWD/.splintr-native/lib\" -lsplintr_go $(cat .splintr-native/lib/native-static-libs.txt)" go build -a ./...
```

See [release instructions](docs/releases.md) for checksum verification and Windows setup.

## Quick Start

```go
package main

import (
	"fmt"
	"log"

	"github.com/maxdswain/splintr-go"
)

func main() {
	// Load a pretrained vocabulary (OpenAI GPT-4/3.5).
	// Also: llama3, deepseek_v3, qwen3, glm4, gpt-oss.
	tok, err := splintr.FromPretrained("cl100k_base")
	if err != nil {
		log.Fatal(err)
	}
	defer tok.Close()

	// Encode and decode.
	ids, err := tok.Encode("Hello, world!")
	if err != nil {
		log.Fatal(err)
	}
	text, err := tok.Decode(ids)
	if err != nil {
		log.Fatal(err)
	}
	fmt.Println(ids)
	fmt.Println(text) // Hello, world!

	// Encode multiple texts, one call per text.
	texts := []string{"Hello, world!", "How are you?"}
	batch := make([][]uint32, len(texts))
	for i, text := range texts {
		batch[i], err = tok.Encode(text)
		if err != nil {
			log.Fatal(err)
		}
	}
	fmt.Println(batch)
}
```

### Hugging Face tokenizer.json

```go
tok, err := splintr.FromFile("tokenizer.json") // or splintr.FromBytes(data)
if err != nil {
	log.Fatal(err)
}
defer tok.Close()
```

### Untrusted text

```go
ids, err := tok.EncodeWithSpecialMode("<|im_start|>user", splintr.SpecialOrdinary)
if err != nil {
	log.Fatal(err)
}
fmt.Println(ids)
```

`SpecialOrdinary` treats control-token spellings as text. Both encoding modes still apply post-processing. Tokenizers support concurrent calls and explicit cleanup with `Close`.

## Development

Source builds need Rust, Go, Make and a C compiler. License collection and release packaging also need `jq` and `tar`.

```sh
make build
make test
make fmt-check lint # rustfmt, Clippy and Revive via golangci-lint v2
```

- [Pretrained names and compatibility](docs/pretrained.md)
- [Build options and layout](docs/development.md)
- [Release packaging](docs/releases.md)

## License

The Go bindings are licensed under [Apache 2.0](LICENSE). Bundled tokenizer vocabularies keep their original licenses, listed in [LICENSE-OTHERS](LICENSE-OTHERS).

Llama 3 vocabulary attribution (required by Meta’s license): “Built with Llama”.
