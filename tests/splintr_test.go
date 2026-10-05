package splintr_test

import (
	"errors"
	"os"
	"reflect"
	"sync"
	"testing"

	splintr "github.com/maxdswain/splintr-go"
)

func loadWordPiece(t *testing.T) *splintr.Tokenizer {
	t.Helper()
	tok, err := splintr.FromFile("test_data/wordpiece.json")
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		if err := tok.Close(); err != nil {
			t.Errorf("Close: %v", err)
		}
	})
	return tok
}

func requireCode(t *testing.T, err error, code splintr.ErrorCode) {
	t.Helper()
	var nativeErr *splintr.Error
	if !errors.As(err, &nativeErr) {
		t.Fatalf("error %T (%v) is not *Error", err, err)
	}
	if nativeErr.Code != code {
		t.Fatalf("error code = %d, want %d (%v)", nativeErr.Code, code, err)
	}
	if nativeErr.Message == "" {
		t.Fatalf("native error has no message: %v", err)
	}
}

func TestWordPieceEncodeDecodeAndMetadata(t *testing.T) {
	tok := loadWordPiece(t)

	ids, err := tok.Encode("hello tokenizer")
	if err != nil {
		t.Fatal(err)
	}
	if want := []uint32{1, 3, 5, 6, 2}; !reflect.DeepEqual(ids, want) {
		t.Fatalf("Encode = %v, want %v", ids, want)
	}
	text, err := tok.Decode(ids)
	if err != nil {
		t.Fatal(err)
	}
	if text != "hello tokenizer" {
		t.Fatalf("Decode = %q", text)
	}
	rendered, err := tok.DecodeWithSpecialMode(ids, splintr.DecodeRenderSpecial)
	if err != nil {
		t.Fatal(err)
	}
	if rendered != "[CLS] hello tokenizer [SEP]" {
		t.Fatalf("rendered Decode = %q", rendered)
	}

	size, err := tok.VocabSize()
	if err != nil || size != 7 {
		t.Fatalf("VocabSize = %d, %v", size, err)
	}
	family, err := tok.Family()
	if err != nil || family != "WordPiece" {
		t.Fatalf("Family = %q, %v", family, err)
	}
	id, found, err := tok.EOSTokenID()
	if err != nil || !found || id != 2 {
		t.Fatalf("EOSTokenID = %d, %v, %v", id, found, err)
	}
	id, found, err = tok.SpecialTokenID("[CLS]")
	if err != nil || !found || id != 1 {
		t.Fatalf("SpecialTokenID = %d, %v, %v", id, found, err)
	}
	id, found, err = tok.SpecialTokenID("missing")
	if err != nil || found || id != 0 {
		t.Fatalf("missing SpecialTokenID = %d, %v, %v", id, found, err)
	}
}

func TestSpecialModesAndPostProcessing(t *testing.T) {
	tok := loadWordPiece(t)

	all, err := tok.Encode("[SEP]")
	if err != nil {
		t.Fatal(err)
	}
	explicitAll, err := tok.EncodeWithSpecialMode("[SEP]", splintr.SpecialAll)
	if err != nil {
		t.Fatal(err)
	}
	ordinary, err := tok.EncodeWithSpecialMode("[SEP]", splintr.SpecialOrdinary)
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(all, explicitAll) {
		t.Fatalf("default = %v, SpecialAll = %v", all, explicitAll)
	}
	if reflect.DeepEqual(all, ordinary) {
		t.Fatalf("SpecialAll and SpecialOrdinary unexpectedly equal: %v", all)
	}
	if all[0] != 1 || all[len(all)-1] != 2 || ordinary[0] != 1 || ordinary[len(ordinary)-1] != 2 {
		t.Fatalf("post-processor missing: all=%v ordinary=%v", all, ordinary)
	}

	empty, err := tok.Encode("")
	if err != nil || !reflect.DeepEqual(empty, []uint32{1, 2}) {
		t.Fatalf("Encode(empty) = %v, %v", empty, err)
	}
	decoded, err := tok.Decode(nil)
	if err != nil || decoded != "" {
		t.Fatalf("Decode(nil) = %q, %v", decoded, err)
	}
}

func TestByteLevelPreservesNULAndUnicode(t *testing.T) {
	tok, err := splintr.FromFile("test_data/bytelevel.json")
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		if err := tok.Close(); err != nil {
			t.Errorf("Close: %v", err)
		}
	})

	input := "a\x00é🙂"
	ids, err := tok.Encode(input)
	if err != nil {
		t.Fatal(err)
	}
	if want := []uint32{0, 1, 2, 3, 4, 5, 6, 7}; !reflect.DeepEqual(ids, want) {
		t.Fatalf("Encode = %v, want %v", ids, want)
	}
	output, err := tok.Decode(ids)
	if err != nil {
		t.Fatal(err)
	}
	if output != input {
		t.Fatalf("round trip = %q (% x), want %q (% x)", output, output, input, input)
	}
	family, err := tok.Family()
	if err != nil || family != "BPE" {
		t.Fatalf("Family = %q, %v", family, err)
	}
}

func TestConstructorsAndTypedErrors(t *testing.T) {
	data, err := os.ReadFile("test_data/wordpiece.json")
	if err != nil {
		t.Fatal(err)
	}
	tok, err := splintr.FromBytes(data)
	if err != nil {
		t.Fatal(err)
	}
	if err := tok.Close(); err != nil {
		t.Fatalf("Close: %v", err)
	}

	_, err = splintr.FromBytes(nil)
	requireCode(t, err, splintr.CodeInvalidArgument)
	_, err = splintr.FromBytes([]byte("{not json"))
	requireCode(t, err, splintr.CodeTokenizer)
	_, err = splintr.FromPretrained("")
	requireCode(t, err, splintr.CodeInvalidArgument)
	if _, err = splintr.FromFile("test_data/does-not-exist.json"); !errors.Is(err, os.ErrNotExist) {
		t.Fatalf("FromFile error = %v", err)
	}
}

func TestOperationErrors(t *testing.T) {
	tok := loadWordPiece(t)

	_, err := tok.Encode(string([]byte{0xff}))
	requireCode(t, err, splintr.CodeInvalidUTF8)
	_, err = tok.EncodeWithSpecialMode("hello", splintr.EncodeSpecialMode(99))
	requireCode(t, err, splintr.CodeInvalidArgument)
	invalidTok, loadErr := splintr.FromBytes([]byte(`{
		"added_tokens":[{"id":0,"content":"[UNK]","special":true}],
		"pre_tokenizer":{"type":"BertPreTokenizer"},
		"model":{"type":"WordPiece","unk_token":"[UNK]","continuing_subword_prefix":"##",
			"max_input_chars_per_word":100,"vocab":{"[UNK]":0,"hello":1}}
	}`))
	if loadErr != nil {
		t.Fatal(loadErr)
	}
	t.Cleanup(func() {
		if err := invalidTok.Close(); err != nil {
			t.Errorf("Close invalid tokenizer: %v", err)
		}
	})
	_, err = invalidTok.Decode([]uint32{999})
	requireCode(t, err, splintr.CodeTokenizer)
	_, err = tok.DecodeWithSpecialMode(nil, splintr.DecodeSpecialMode(99))
	requireCode(t, err, splintr.CodeInvalidArgument)
	_, _, err = tok.SpecialTokenID("")
	requireCode(t, err, splintr.CodeInvalidArgument)
	_, _, err = tok.SpecialTokenID(string([]byte{0xff}))
	requireCode(t, err, splintr.CodeInvalidUTF8)
}

func TestCloseCopiedAndZeroValue(t *testing.T) {
	tok := loadWordPiece(t)
	copied := *tok
	if err := copied.Close(); err != nil {
		t.Fatal(err)
	}
	if err := tok.Close(); err != nil {
		t.Fatalf("second Close: %v", err)
	}
	if _, err := tok.Encode("hello"); !errors.Is(err, splintr.ErrClosed) {
		t.Fatalf("original after copied Close: %v", err)
	}
	if _, err := copied.VocabSize(); !errors.Is(err, splintr.ErrClosed) {
		t.Fatalf("copy after Close: %v", err)
	}

	var zero splintr.Tokenizer
	if err := zero.Close(); err != nil {
		t.Fatalf("zero Close: %v", err)
	}
	if _, err := zero.Decode(nil); !errors.Is(err, splintr.ErrClosed) {
		t.Fatalf("zero Decode: %v", err)
	}
	var nilTokenizer *splintr.Tokenizer
	if err := nilTokenizer.Close(); err != nil {
		t.Fatalf("nil Close: %v", err)
	}
	if _, err := nilTokenizer.Encode(""); !errors.Is(err, splintr.ErrClosed) {
		t.Fatalf("nil Encode: %v", err)
	}
}

func TestConcurrentUse(t *testing.T) {
	tok := loadWordPiece(t)
	var wg sync.WaitGroup
	for i := 0; i < 16; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for j := 0; j < 20; j++ {
				ids, err := tok.Encode("hello world")
				if err != nil {
					t.Error(err)
					return
				}
				text, err := tok.Decode(ids)
				if err != nil || text != "hello world" {
					t.Errorf("concurrent Decode = %q, %v", text, err)
					return
				}
			}
		}()
	}
	wg.Wait()
}

func TestConcurrentUseAndClose(t *testing.T) {
	tok, err := splintr.FromFile("test_data/wordpiece.json")
	if err != nil {
		t.Fatal(err)
	}

	start := make(chan struct{})
	var ready sync.WaitGroup
	ready.Add(16)
	var wg sync.WaitGroup
	errorsSeen := make(chan error, 16)
	for i := 0; i < 16; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			<-start
			_, err := tok.Encode("hello world")
			ready.Done()
			if err != nil {
				errorsSeen <- err
				return
			}
			for j := 0; j < 100; j++ {
				ids, err := tok.Encode("hello world")
				if errors.Is(err, splintr.ErrClosed) {
					return
				}
				if err != nil {
					errorsSeen <- err
					return
				}
				if _, err = tok.Decode(ids); err != nil && !errors.Is(err, splintr.ErrClosed) {
					errorsSeen <- err
					return
				}
			}
		}()
	}
	close(start)
	ready.Wait()
	if err := tok.Close(); err != nil {
		t.Fatal(err)
	}
	wg.Wait()
	close(errorsSeen)
	for err := range errorsSeen {
		t.Errorf("concurrent operation: %v", err)
	}
	if _, err := tok.Family(); !errors.Is(err, splintr.ErrClosed) {
		t.Fatalf("Family after Close: %v", err)
	}
}
