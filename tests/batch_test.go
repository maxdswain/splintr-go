package splintr_test

import (
	"errors"
	"reflect"
	"strings"
	"sync"
	"testing"

	splintr "github.com/maxdswain/splintr-go"
)

func TestBatchDefaultsAndEmptyItems(t *testing.T) {
	tok := loadWordPiece(t)
	for _, input := range [][]string{nil, {}} {
		got, err := tok.EncodeBatch(input)
		if err != nil || got == nil || len(got) != 0 {
			t.Fatalf("EncodeBatch(%v) = %v, %v", input, got, err)
		}
	}
	for _, input := range [][][]uint32{nil, {}} {
		got, err := tok.DecodeBatch(input)
		if err != nil || got == nil || len(got) != 0 {
			t.Fatalf("DecodeBatch(%v) = %v, %v", input, got, err)
		}
	}

	texts := []string{"hello tokenizer", "", "[SEP]", "world", "hello", "[UNK]"}
	encoded, err := tok.EncodeBatch(texts)
	if err != nil || len(encoded) != len(texts) {
		t.Fatalf("EncodeBatch = %v, %v", encoded, err)
	}
	for i, text := range texts {
		want, singleErr := tok.Encode(text)
		if singleErr != nil || !reflect.DeepEqual(encoded[i], want) {
			t.Fatalf("EncodeBatch[%d] = %v, want %v (single error %v)", i, encoded[i], want, singleErr)
		}
		if encoded[i][0] != 1 || encoded[i][len(encoded[i])-1] != 2 {
			t.Fatalf("post-processor missing at %d: %v", i, encoded[i])
		}
	}
	ordinary, err := tok.EncodeWithSpecialMode("[SEP]", splintr.SpecialOrdinary)
	if err != nil || reflect.DeepEqual(encoded[2], ordinary) {
		t.Fatalf("batch must recognize special spelling: %v, ordinary %v, %v", encoded[2], ordinary, err)
	}

	lists := [][]uint32{nil, {}, encoded[0], {1, 3, 2}, {1, 2}, {999, 3, 999}}
	decoded, err := tok.DecodeBatch(lists)
	if err != nil || len(decoded) != len(lists) {
		t.Fatalf("DecodeBatch = %v, %v", decoded, err)
	}
	for i, ids := range lists {
		want, singleErr := tok.Decode(ids)
		if singleErr != nil || decoded[i] != want {
			t.Fatalf("DecodeBatch[%d] = %q, want %q (single error %v)", i, decoded[i], want, singleErr)
		}
	}
	if decoded[0] != "" || decoded[1] != "" || decoded[4] != "" || decoded[3] != "hello" || decoded[5] != "hello" {
		t.Fatalf("skip-special/decoder-pipeline result = %q", decoded)
	}
}

func TestBatchByteLevelAndInvalidUTF8(t *testing.T) {
	tok, err := splintr.FromFile("test_data/bytelevel.json")
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = tok.Close() })
	emptyIDs, err := tok.EncodeBatch([]string{"", "", ""})
	if err != nil || !reflect.DeepEqual(emptyIDs, [][]uint32{{}, {}, {}}) {
		t.Fatalf("empty payload batch = %v, %v", emptyIDs, err)
	}
	emptyText, err := tok.DecodeBatch([][]uint32{nil, {}, nil})
	if err != nil || !reflect.DeepEqual(emptyText, []string{"", "", ""}) {
		t.Fatalf("empty payload decode = %q, %v", emptyText, err)
	}
	texts := []string{"", "a\x00é🙂", "a", "é", "a\x00é🙂"}
	ids, err := tok.EncodeBatch(texts)
	if err != nil || len(ids) != len(texts) {
		t.Fatalf("EncodeBatch = %v, %v", ids, err)
	}
	for i, text := range texts {
		want, singleErr := tok.Encode(text)
		if singleErr != nil || !reflect.DeepEqual(ids[i], want) {
			t.Fatalf("item %d = %v, want %v (single error %v)", i, ids[i], want, singleErr)
		}
	}
	decoded, err := tok.DecodeBatch(ids)
	if err != nil || !reflect.DeepEqual(decoded, texts) {
		t.Fatalf("DecodeBatch = %q, %v; want %q", decoded, err, texts)
	}
	// The two invalid strings form valid UTF-8 only if incorrectly concatenated.
	for _, input := range [][]string{{"a", string([]byte{0xff}), "a"}, {string([]byte{0xc3}), string([]byte{0xa9})}} {
		got, batchErr := tok.EncodeBatch(input)
		if got != nil {
			t.Fatalf("invalid UTF-8 returned partial batch: %v", got)
		}
		requireCode(t, batchErr, splintr.CodeInvalidUTF8)
	}
}

func TestBatchDecodeFailureIsAtomic(t *testing.T) {
	// Unlike the decoder-equipped WordPiece fixture, this decoderless tokenizer
	// reports unknown IDs instead of silently omitting them.
	tok, err := splintr.FromBytes([]byte(`{
		"added_tokens":[{"id":0,"content":"[UNK]","special":true}],
		"pre_tokenizer":{"type":"BertPreTokenizer"},
		"model":{"type":"WordPiece","unk_token":"[UNK]","continuing_subword_prefix":"##",
			"max_input_chars_per_word":100,"vocab":{"[UNK]":0,"hello":1}}
	}`))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = tok.Close() })
	_, singleErr := tok.Decode([]uint32{999})
	requireCode(t, singleErr, splintr.CodeTokenizer)
	got, batchErr := tok.DecodeBatch([][]uint32{{1}, nil, {999}, {1}})
	if got != nil {
		t.Fatalf("failed batch returned partial output: %v", got)
	}
	requireCode(t, batchErr, splintr.CodeTokenizer)

	valid := make([]uint32, 1024)
	for i := range valid {
		valid[i] = 1
	}
	parallel := make([][]uint32, 16)
	for i := range parallel {
		parallel[i] = valid
	}
	parallel[len(parallel)-1] = []uint32{999}
	got, batchErr = tok.DecodeBatch(parallel)
	if got != nil {
		t.Fatalf("failed parallel batch returned partial output: %v", got)
	}
	requireCode(t, batchErr, splintr.CodeTokenizer)
}

func TestBatchOwnedOutputAndRepeatedCalls(t *testing.T) {
	tok := loadWordPiece(t)
	input := []string{"hello", "world", "hello tokenizer"}
	first, err := tok.EncodeBatch(input)
	if err != nil {
		t.Fatal(err)
	}
	second, err := tok.EncodeBatch(input)
	if err != nil || !reflect.DeepEqual(first, second) {
		t.Fatalf("repeated EncodeBatch = %v, %v", second, err)
	}
	decoded, err := tok.DecodeBatch(first)
	if err != nil || !reflect.DeepEqual(decoded, input) {
		t.Fatalf("DecodeBatch = %q, %v", decoded, err)
	}
	// Touch the entire spare capacity, not just len: another row must not
	// share backing storage even when the first row is appended to.
	firstRow := first[0][:cap(first[0])]
	for j := range firstRow {
		firstRow[j] = 999
	}
	if !reflect.DeepEqual(first[1], []uint32{1, 4, 2}) ||
		!reflect.DeepEqual(first[2], []uint32{1, 3, 5, 6, 2}) {
		t.Fatalf("batch rows share capacity: %v", first)
	}
	if !reflect.DeepEqual(second, [][]uint32{{1, 3, 2}, {1, 4, 2}, {1, 3, 5, 6, 2}}) {
		t.Fatalf("earlier batch corrupted later batch: %v", second)
	}
	again, err := tok.EncodeBatch(input)
	if err != nil || !reflect.DeepEqual(again, second) {
		t.Fatalf("native output reused after free: %v, %v", again, err)
	}
	if !reflect.DeepEqual(decoded, input) {
		t.Fatalf("decoded output changed after later call: %q", decoded)
	}
	if err := tok.Close(); err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(again, second) || !reflect.DeepEqual(decoded, input) {
		t.Fatalf("output changed after Close: %v, %q", again, decoded)
	}
}

func TestBatchLargeInputs(t *testing.T) {
	tok := loadWordPiece(t)
	// Both directions exceed the 32 KiB native parallel threshold independently.
	texts := make([]string, 128)
	lists := make([][]uint32, len(texts))
	for i := range texts {
		texts[i] = strings.Repeat("hello world ", 28+i) + "tokenizer"
		lists[i] = make([]uint32, 300+i)
		for j := range lists[i] {
			lists[i][j] = uint32(3 + (i+j)%2)
		}
	}
	encoded, err := tok.EncodeBatch(texts)
	if err != nil || len(encoded) != len(texts) {
		t.Fatalf("large EncodeBatch: %d items, %v", len(encoded), err)
	}
	decoded, err := tok.DecodeBatch(lists)
	if err != nil || len(decoded) != len(lists) {
		t.Fatalf("large DecodeBatch: %d items, %v", len(decoded), err)
	}
	for i := range texts {
		wantIDs, encodeErr := tok.Encode(texts[i])
		wantText, decodeErr := tok.Decode(lists[i])
		if encodeErr != nil || decodeErr != nil {
			t.Fatalf("single item %d: encode %v, decode %v", i, encodeErr, decodeErr)
		}
		if !reflect.DeepEqual(encoded[i], wantIDs) || decoded[i] != wantText {
			t.Fatalf("parallel item %d differs from single calls", i)
		}
	}
}

func TestBatchClosedHandles(t *testing.T) {
	tok := loadWordPiece(t)
	copyTok := *tok
	if err := copyTok.Close(); err != nil {
		t.Fatal(err)
	}
	var zero splintr.Tokenizer
	var nilTok *splintr.Tokenizer
	for _, handle := range []*splintr.Tokenizer{tok, &copyTok, &zero, nilTok} {
		for _, texts := range [][]string{nil, {"hello"}} {
			if got, err := handle.EncodeBatch(texts); got != nil || !errors.Is(err, splintr.ErrClosed) {
				t.Errorf("closed EncodeBatch(%v) = %v, %v", texts, got, err)
			}
		}
		for _, ids := range [][][]uint32{nil, {{3}}} {
			if got, err := handle.DecodeBatch(ids); got != nil || !errors.Is(err, splintr.ErrClosed) {
				t.Errorf("closed DecodeBatch(%v) = %v, %v", ids, got, err)
			}
		}
	}
}

func TestBatchConcurrentUse(t *testing.T) {
	tok := loadWordPiece(t)
	var workers sync.WaitGroup
	for range 12 {
		workers.Add(1)
		go func() {
			defer workers.Done()
			for range 20 {
				ids, err := tok.EncodeBatch([]string{"hello", "world", "hello tokenizer"})
				if err != nil {
					t.Errorf("concurrent EncodeBatch: %v", err)
					return
				}
				text, err := tok.DecodeBatch(ids)
				if err != nil || !reflect.DeepEqual(text, []string{"hello", "world", "hello tokenizer"}) {
					t.Errorf("concurrent DecodeBatch = %q, %v", text, err)
					return
				}
			}
		}()
	}
	workers.Wait()
}

func TestBatchConcurrentUseAndClose(t *testing.T) {
	tok, err := splintr.FromFile("test_data/wordpiece.json")
	if err != nil {
		t.Fatal(err)
	}
	start := make(chan struct{})
	var ready sync.WaitGroup
	var workers sync.WaitGroup
	ready.Add(12)
	for range 12 {
		workers.Add(1)
		go func() {
			defer workers.Done()
			<-start
			ids, encodeErr := tok.EncodeBatch([]string{"hello", "world", "hello tokenizer"})
			ready.Done()
			if encodeErr != nil && !errors.Is(encodeErr, splintr.ErrClosed) {
				t.Errorf("EncodeBatch during Close: %v", encodeErr)
				return
			}
			for range 30 {
				if encodeErr == nil {
					text, decodeErr := tok.DecodeBatch(ids)
					if decodeErr != nil && !errors.Is(decodeErr, splintr.ErrClosed) {
						t.Errorf("DecodeBatch during Close: %v", decodeErr)
						return
					}
					if decodeErr == nil && !reflect.DeepEqual(text, []string{"hello", "world", "hello tokenizer"}) {
						t.Errorf("DecodeBatch during Close = %q", text)
						return
					}
				}
				ids, encodeErr = tok.EncodeBatch([]string{"hello", "world", "hello tokenizer"})
				if errors.Is(encodeErr, splintr.ErrClosed) {
					return
				}
				if encodeErr != nil {
					t.Errorf("EncodeBatch during Close: %v", encodeErr)
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
	workers.Wait()
	if got, err := tok.DecodeBatch(nil); got != nil || !errors.Is(err, splintr.ErrClosed) {
		t.Fatalf("DecodeBatch after Close = %v, %v", got, err)
	}
}
