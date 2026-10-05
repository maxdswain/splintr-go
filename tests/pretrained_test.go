//go:build bundled

package splintr_test

import (
	"reflect"
	"testing"

	splintr "github.com/maxdswain/splintr-go"
)

// Keep this list in enum order with splintr 0.21's PretrainedVocab. Each entry
// is a distinct vocabulary, rather than another accepted spelling of one.
var canonicalPretrainedNames = []string{
	"cl100k_base",
	"o200k_base",
	"llama3",
	"deepseek_v3",
	"qwen3",
	"glm4",
	"gpt-oss",
	"phi4",
	"olmo2",
	"llama2",
	"codellama",
	"modernbert",
	"gemma2",
	"gemma3",
	"gemma4",
	"kimi_k2",
	"kimi_k3",
	"mistral_v1",
	"mistral_v2",
	"mistral_v3",
	"whisper_v1",
	"whisper_v2",
	"whisper_v3",
}

func TestBundledCanonicalPretrained(t *testing.T) {
	const input = "Hello, tokenizer 123!"

	// Deliberately run sequentially and close each tokenizer before loading the
	// next: all 23 vocabularies are large when resident at the same time.
	for _, name := range canonicalPretrainedNames {
		t.Run(name, func(t *testing.T) {
			tok, err := splintr.FromPretrained(name)
			if err != nil {
				t.Fatalf("FromPretrained(%q): %v", name, err)
			}

			ids, encodeErr := tok.EncodeWithSpecialMode(input, splintr.SpecialOrdinary)
			decoded, decodeErr := tok.Decode(ids)
			size, sizeErr := tok.VocabSize()
			family, familyErr := tok.Family()
			_, _, eosErr := tok.EOSTokenID()
			closeErr := tok.Close()

			if encodeErr != nil {
				t.Fatalf("EncodeWithSpecialMode: %v", encodeErr)
			}
			if len(ids) == 0 {
				t.Fatal("EncodeWithSpecialMode returned no IDs")
			}
			if decodeErr != nil {
				t.Fatalf("Decode: %v", decodeErr)
			}
			// Some families normalize ordinary input, so exact round trips are
			// intentionally not asserted.
			if decoded == "" {
				t.Fatal("Decode returned empty text")
			}
			if sizeErr != nil || size <= 0 {
				t.Fatalf("VocabSize = %d, %v", size, sizeErr)
			}
			if familyErr != nil || family == "" || family == "Unknown" {
				t.Fatalf("Family = %q, %v", family, familyErr)
			}
			if eosErr != nil {
				t.Fatalf("EOSTokenID: %v", eosErr)
			}
			if closeErr != nil {
				t.Fatalf("Close: %v", closeErr)
			}
		})
	}
}

func TestBundledKnownSamples(t *testing.T) {
	tests := []struct {
		name string
		want []uint32
	}{
		{"qwen3", []uint32{9707, 1879}},
		{"cl100k_base", []uint32{9906, 1917}},
		{"gemma3", []uint32{9259, 1902}},
		{"mistral_v3", []uint32{22177, 4304}},
		{"deepseek_v3", []uint32{19923, 2058}},
	}

	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			tok, err := splintr.FromPretrained(test.name)
			if err != nil {
				t.Fatalf("FromPretrained(%q): %v", test.name, err)
			}
			ids, encodeErr := tok.EncodeWithSpecialMode("Hello world", splintr.SpecialOrdinary)
			closeErr := tok.Close()
			if encodeErr != nil {
				t.Fatalf("EncodeWithSpecialMode: %v", encodeErr)
			}
			if !reflect.DeepEqual(ids, test.want) {
				t.Fatalf("EncodeWithSpecialMode = %v, want %v", ids, test.want)
			}
			if closeErr != nil {
				t.Fatalf("Close: %v", closeErr)
			}
		})
	}
}

func TestBundledPretrainedAliases(t *testing.T) {
	aliases := []string{
		"llama3.1",
		"deepseek-v3",
		"qwen2.5",
		"glm-4.5",
		"gpt_oss",
		"phi-4",
		"olmo-2",
		"tinyllama",
		"code-llama",
		"modern-bert",
		"gemma-2",
		"embeddinggemma",
		"gemma-4",
		"kimi",
		"kimi-k3",
		"mistral",
		"whisper",
		"whisper-large-v3",
	}

	for _, name := range aliases {
		t.Run(name, func(t *testing.T) {
			tok, err := splintr.FromPretrained(name)
			if err != nil {
				t.Fatalf("FromPretrained(%q): %v", name, err)
			}
			ids, encodeErr := tok.EncodeWithSpecialMode("Alias check", splintr.SpecialOrdinary)
			closeErr := tok.Close()
			if encodeErr != nil {
				t.Fatalf("EncodeWithSpecialMode: %v", encodeErr)
			}
			if len(ids) == 0 {
				t.Fatal("EncodeWithSpecialMode returned no IDs")
			}
			if closeErr != nil {
				t.Fatalf("Close: %v", closeErr)
			}
		})
	}
}
