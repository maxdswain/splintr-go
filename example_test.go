package splintr_test

import (
	"errors"
	"fmt"
	"slices"

	"github.com/maxdswain/splintr-go"
)

const tokenizerJSON = `{
	"added_tokens": [{"id": 3, "content": "[SEP]", "special": true}],
	"pre_tokenizer": {"type": "Whitespace"},
	"decoder": {"type": "WordPiece", "prefix": "##", "cleanup": true},
	"model": {"type": "WordPiece", "unk_token": "[UNK]",
		"continuing_subword_prefix": "##", "max_input_chars_per_word": 100,
		"vocab": {"[UNK]": 0, "hello": 1, "world": 2, "[SEP]": 3}}
}`

func exampleTokenizer() *splintr.Tokenizer {
	tok, err := splintr.FromBytes([]byte(tokenizerJSON))
	if err != nil {
		panic(err)
	}
	return tok
}

// This example uses a small JSON vocabulary and needs no bundled vocabularies.
func Example() {
	tok := exampleTokenizer()
	defer tok.Close()

	ids, err := tok.Encode("hello world")
	if err != nil {
		panic(err)
	}
	text, err := tok.Decode(ids)
	if err != nil {
		panic(err)
	}
	fmt.Println(ids)
	fmt.Println(text)
	// Output:
	// [1 2]
	// hello world
}

func ExampleTokenizer_EncodeBatch() {
	tok := exampleTokenizer()
	defer tok.Close()

	batch, err := tok.EncodeBatch([]string{"hello world", "world", ""})
	if err != nil {
		panic(err)
	}
	texts, err := tok.DecodeBatch(batch)
	if err != nil {
		panic(err)
	}
	fmt.Println(batch)
	fmt.Printf("%q\n", texts)
	// Output:
	// [[1 2] [2] []]
	// ["hello world" "world" ""]
}

func ExampleStreamingDecoder() {
	tok := exampleTokenizer()
	defer tok.Close()

	stream, err := tok.StreamingDecoder()
	if err != nil {
		panic(err)
	}
	defer stream.Close()

	for _, id := range []uint32{1, 2} {
		text, err := stream.AddToken(id)
		if err != nil {
			panic(err)
		}
		fmt.Print(text)
	}
	tail, err := stream.Flush()
	if err != nil {
		panic(err)
	}
	fmt.Println(tail)
	// Output: hello world
}

func ExampleTokenizer_EncodeWithSpecialMode() {
	tok := exampleTokenizer()
	defer tok.Close()

	id, found, err := tok.SpecialTokenID("[SEP]")
	if err != nil {
		panic(err)
	}
	if !found {
		panic("missing special token")
	}
	special, err := tok.EncodeWithSpecialMode("hello [SEP]", splintr.SpecialAll)
	if err != nil {
		panic(err)
	}
	ordinary, err := tok.EncodeWithSpecialMode("hello [SEP]", splintr.SpecialOrdinary)
	if err != nil {
		panic(err)
	}
	fmt.Println("recognized:", slices.Contains(special, id))
	fmt.Println("ordinary:", slices.Contains(ordinary, id))

	text, err := tok.DecodeWithSpecialMode(special, splintr.DecodeRenderSpecial)
	if err != nil {
		panic(err)
	}
	fmt.Println(text)
	// Output:
	// recognized: true
	// ordinary: false
	// hello [SEP]
}

func ExampleError() {
	_, err := splintr.FromBytes(nil)
	if nativeErr, ok := errors.AsType[*splintr.Error](err); ok {
		fmt.Println("invalid argument:", nativeErr.Code == splintr.ErrorInvalidArgument)
	}

	var tok splintr.Tokenizer
	_, err = tok.Encode("hello")
	fmt.Println("closed:", errors.Is(err, splintr.ErrClosed))
	// Output:
	// invalid argument: true
	// closed: true
}
