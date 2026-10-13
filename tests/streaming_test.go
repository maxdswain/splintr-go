package splintr_test

import (
	"errors"
	"os"
	"runtime"
	"strconv"
	"strings"
	"sync"
	"testing"

	splintr "github.com/maxdswain/splintr-go"
)

func loadByteLevel(t *testing.T) *splintr.Tokenizer {
	t.Helper()
	tok, err := splintr.FromFile("test_data/bytelevel.json")
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = tok.Close() })
	return tok
}

func newStream(t *testing.T, tok *splintr.Tokenizer) *splintr.StreamingDecoder {
	t.Helper()
	d, err := tok.StreamingDecoder()
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = d.Close() })
	return d
}

func streamAll(t *testing.T, d *splintr.StreamingDecoder, ids []uint32, group int) string {
	t.Helper()
	var result strings.Builder
	for start := 0; start < len(ids); start += group {
		end := min(start+group, len(ids))
		text, err := d.AddTokens(ids[start:end])
		if err != nil {
			t.Fatalf("AddTokens(%v): %v", ids[start:end], err)
		}
		result.WriteString(text)
	}
	text, err := d.Flush()
	if err != nil {
		t.Fatal(err)
	}
	result.WriteString(text)
	return result.String()
}

func TestStreamingByteLevelGroupingAndPending(t *testing.T) {
	tok := loadByteLevel(t)
	ids := []uint32{0, 1, 2, 3, 4, 5, 6, 7}
	whole, err := tok.Decode(ids)
	if err != nil || whole != "a\x00é🙂" {
		t.Fatalf("Decode = %q, %v", whole, err)
	}
	for _, group := range []int{1, 2, 3, 8} {
		d := newStream(t, tok)
		if got := streamAll(t, d, ids, group); got != whole {
			t.Fatalf("group %d: %q != %q", group, got, whole)
		}
	}
	d := newStream(t, tok)
	for _, step := range []struct {
		id      uint32
		text    string
		pending int
	}{
		{2, "", 1}, {3, "é", 0}, {4, "", 1}, {5, "", 2},
		{6, "", 3}, {7, "🙂", 0}, {1, "\x00", 0},
	} {
		text, err := d.AddToken(step.id)
		if err != nil || text != step.text {
			t.Fatalf("AddToken(%d) = %q, %v; want %q", step.id, text, err, step.text)
		}
		count, err := d.PendingBytes()
		if err != nil || count != step.pending {
			t.Fatalf("PendingBytes = %d, %v; want %d", count, err, step.pending)
		}
		pending, err := d.HasPending()
		if err != nil || pending != (count > 0) {
			t.Fatalf("HasPending = %v, %v", pending, err)
		}
	}
	if got, err := d.AddTokens(nil); err != nil || got != "" {
		t.Fatalf("AddTokens(nil) = %q, %v", got, err)
	}
	if got, err := d.Flush(); err != nil || got != "" {
		t.Fatalf("empty Flush = %q, %v", got, err)
	}
}

func TestStreamingFlushResetAndLossy(t *testing.T) {
	d := newStream(t, loadByteLevel(t))
	if text, err := d.AddToken(2); err != nil || text != "" {
		t.Fatalf("partial = %q, %v", text, err)
	}
	if text, err := d.Flush(); err != nil || text != "�" {
		t.Fatalf("incomplete Flush = %q, %v", text, err)
	}
	if pending, err := d.HasPending(); err != nil || pending {
		t.Fatalf("after Flush HasPending = %v, %v", pending, err)
	}
	if text, err := d.AddToken(0); err != nil || text != "a" {
		t.Fatalf("after Flush AddToken = %q, %v", text, err)
	}
	if _, err := d.AddToken(2); err != nil {
		t.Fatal(err)
	}
	if err := d.Reset(); err != nil {
		t.Fatal(err)
	}
	if count, err := d.PendingBytes(); err != nil || count != 0 {
		t.Fatalf("after Reset PendingBytes = %d, %v", count, err)
	}
	if text, err := d.AddTokensLossy([]uint32{0, 999, 1}); err != nil || text != "a\x00" {
		t.Fatalf("AddTokensLossy = %q, %v", text, err)
	}
	if text, err := d.AddTokenLossy(999); err != nil || text != "" {
		t.Fatalf("AddTokenLossy = %q, %v", text, err)
	}
	if text, err := d.Flush(); err != nil || text != "" {
		t.Fatalf("Flush after lossy = %q, %v", text, err)
	}
}

func TestStreamingPostprocessingPendingAndReset(t *testing.T) {
	// No declared pipeline: backend cleanup buffers spaces across token boundaries.
	tok, err := splintr.FromBytes([]byte(`{
		"model": {"type": "WordPiece", "unk_token": "[UNK]", "continuing_subword_prefix": "##",
			"max_input_chars_per_word": 100, "vocab": {"[UNK]": 0, "hello ": 1, "##,": 2, "world": 3}}
	}`))
	if err != nil {
		t.Fatal(err)
	}
	defer tok.Close()
	d := newStream(t, tok)
	if text, err := d.AddToken(1); err != nil || text != "hello" {
		t.Fatalf("held space = %q, %v", text, err)
	}
	if count, err := d.PendingBytes(); err != nil || count != 1 {
		t.Fatalf("held space PendingBytes = %d, %v", count, err)
	}
	if pending, err := d.HasPending(); err != nil || !pending {
		t.Fatalf("held space HasPending = %v, %v", pending, err)
	}
	if text, err := d.AddToken(2); err != nil || text != "," {
		t.Fatalf("cleanup across chunks = %q, %v", text, err)
	}
	if text, err := d.Flush(); err != nil || text != "" {
		t.Fatalf("Flush after punctuation = %q, %v", text, err)
	}
	if text, err := d.AddToken(3); err != nil || text != " world" {
		t.Fatalf("Flush must not reset word position: %q, %v", text, err)
	}
	if _, err := d.AddToken(1); err != nil {
		t.Fatal(err)
	}
	if err := d.Reset(); err != nil {
		t.Fatal(err)
	}
	if text, err := d.AddToken(3); err != nil || text != "world" {
		t.Fatalf("Reset must reset word position: %q, %v", text, err)
	}
}

func TestStreamingStrictErrorAcceptedPrefix(t *testing.T) {
	d := newStream(t, loadByteLevel(t))
	_, err := d.AddTokens([]uint32{2, 999, 3})
	requireCode(t, err, splintr.CodeTokenizer)
	if count, err := d.PendingBytes(); err != nil || count != 1 {
		t.Fatalf("prefix PendingBytes = %d, %v", count, err)
	}
	if text, err := d.AddToken(3); err != nil || text != "é" {
		t.Fatalf("accepted prefix = %q, %v", text, err)
	}
	if err := d.Reset(); err != nil {
		t.Fatal(err)
	}
}

func TestStreamingSpecialModesAndInvalidPipeline(t *testing.T) {
	tok := loadWordPiece(t)
	ids := []uint32{1, 3, 5, 6, 2}
	for _, mode := range []splintr.DecodeSpecialMode{splintr.DecodeSkipSpecial, splintr.DecodeRenderSpecial} {
		want, err := tok.DecodeWithSpecialMode(ids, mode)
		if err != nil {
			t.Fatal(err)
		}
		d, err := tok.StreamingDecoderWithSpecialMode(mode)
		if err != nil {
			t.Fatal(err)
		}
		for _, group := range []int{1, 2, len(ids)} {
			if got := streamAll(t, d, ids, group); got != want {
				t.Fatalf("mode %d group %d: %q != %q", mode, group, got, want)
			}
			if err := d.Reset(); err != nil {
				t.Fatal(err)
			}
		}
		_ = d.Close()
	}
	modes := []splintr.DecodeSpecialMode{-1, 99}
	if strconv.IntSize == 64 {
		large := uint64(1) << 40
		modes = append(modes, splintr.DecodeSpecialMode(large))
	}
	for _, mode := range modes {
		_, err := tok.StreamingDecoderWithSpecialMode(mode)
		requireCode(t, err, splintr.CodeInvalidArgument)
	}
	data, err := os.ReadFile("test_data/wordpiece.json")
	if err != nil {
		t.Fatal(err)
	}
	// BPEDecoder supports whole-sequence decoding, but not streaming.
	data = []byte(strings.Replace(
		string(data), `"type": "WordPiece", "prefix": "##", "cleanup": true`,
		`"type": "BPEDecoder", "suffix": "</w>"`, 1,
	))
	unsupported, err := splintr.FromBytes(data)
	if err != nil {
		t.Fatal(err)
	}
	defer unsupported.Close()
	_, err = unsupported.StreamingDecoder()
	requireCode(t, err, splintr.CodeTokenizer)
}

func TestStreamingCopiesClosedAndOutlivesParent(t *testing.T) {
	tok := loadByteLevel(t)
	d := newStream(t, tok)
	copyD := *d
	if err := tok.Close(); err != nil {
		t.Fatal(err)
	}
	if text, err := copyD.AddToken(0); err != nil || text != "a" {
		t.Fatalf("stream after parent Close = %q, %v", text, err)
	}
	factories := []func() (*splintr.StreamingDecoder, error){
		tok.StreamingDecoder,
		func() (*splintr.StreamingDecoder, error) {
			return tok.StreamingDecoderWithSpecialMode(splintr.DecodeRenderSpecial)
		},
	}
	for _, factory := range factories {
		if _, err := factory(); !errors.Is(err, splintr.ErrClosed) {
			t.Fatalf("closed factory = %v", err)
		}
	}
	if err := copyD.Close(); err != nil {
		t.Fatal(err)
	}
	if err := d.Close(); err != nil {
		t.Fatal(err)
	}
	assertStreamClosed(t, d)
	assertStreamClosed(t, &copyD)
	var zero splintr.StreamingDecoder
	assertStreamClosed(t, &zero)
	var nilD *splintr.StreamingDecoder
	assertStreamClosed(t, nilD)
	for _, handle := range []*splintr.StreamingDecoder{&zero, nilD} {
		if err := handle.Close(); err != nil {
			t.Fatal(err)
		}
	}
	var nilTok *splintr.Tokenizer
	var zeroTok splintr.Tokenizer
	for _, handle := range []*splintr.Tokenizer{nilTok, &zeroTok} {
		if _, err := handle.StreamingDecoder(); !errors.Is(err, splintr.ErrClosed) {
			t.Fatalf("nil/zero factory: %v", err)
		}
		_, err := handle.StreamingDecoderWithSpecialMode(splintr.DecodeRenderSpecial)
		if !errors.Is(err, splintr.ErrClosed) {
			t.Fatalf("nil/zero mode factory: %v", err)
		}
	}
}

func assertStreamClosed(t *testing.T, d *splintr.StreamingDecoder) {
	t.Helper()
	for _, call := range []func() error{
		func() error { _, err := d.AddToken(0); return err },
		func() error { _, err := d.AddTokens(nil); return err },
		func() error { _, err := d.AddTokenLossy(0); return err },
		func() error { _, err := d.AddTokensLossy(nil); return err },
		func() error { _, err := d.Flush(); return err }, d.Reset,
		func() error { _, err := d.HasPending(); return err },
		func() error { _, err := d.PendingBytes(); return err },
	} {
		if err := call(); !errors.Is(err, splintr.ErrClosed) {
			t.Fatalf("closed stream method: %v", err)
		}
	}
}

func TestStreamingIndependentCursorsAndCopiedHandleGC(t *testing.T) {
	tok := loadByteLevel(t)
	// Cleanup must not keep the original wrapper alive.
	one, err := tok.StreamingDecoder()
	if err != nil {
		t.Fatal(err)
	}
	copyD := *one
	t.Cleanup(func() { _ = copyD.Close() })
	two := newStream(t, tok)
	if _, err := one.AddToken(2); err != nil {
		t.Fatal(err)
	}
	if got, err := two.AddToken(0); err != nil || got != "a" {
		t.Fatalf("independent = %q, %v", got, err)
	}
	one = nil
	runtime.GC()
	runtime.GC()
	if got, err := copyD.AddToken(3); err != nil || got != "é" {
		t.Fatalf("copy survived GC = %q, %v", got, err)
	}
}

func TestStreamingConcurrentCallsAndClose(t *testing.T) {
	tok := loadByteLevel(t)
	d := newStream(t, tok)
	started := make(chan struct{}, 17)
	var wg sync.WaitGroup
	for i := 0; i < 16; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			for j := 0; j < 60; j++ {
				text, err := d.AddTokenLossy(0)
				if j == 0 {
					started <- struct{}{}
				}
				if errors.Is(err, splintr.ErrClosed) {
					return
				}
				if err != nil || text != "a" {
					t.Errorf("concurrent AddTokenLossy = %q, %v", text, err)
					return
				}
				if _, err := d.HasPending(); err != nil && !errors.Is(err, splintr.ErrClosed) {
					t.Error(err)
				}
				if _, err := d.PendingBytes(); err != nil && !errors.Is(err, splintr.ErrClosed) {
					t.Error(err)
				}
				if err := d.Reset(); err != nil && !errors.Is(err, splintr.ErrClosed) {
					t.Error(err)
				}
			}
		}()
	}
	wg.Add(1)
	go func() {
		defer wg.Done()
		for i := 0; i < 80; i++ {
			stream, err := tok.StreamingDecoder()
			if i == 0 {
				started <- struct{}{}
			}
			if errors.Is(err, splintr.ErrClosed) {
				return
			}
			if err != nil {
				t.Error(err)
				return
			}
			if text, err := stream.AddToken(0); err != nil || text != "a" {
				t.Errorf("stream during parent Close = %q, %v", text, err)
			}
			if err := stream.Close(); err != nil {
				t.Error(err)
			}
		}
	}()
	for i := 0; i < cap(started); i++ {
		<-started
	}
	if err := d.Close(); err != nil {
		t.Fatal(err)
	}
	if err := tok.Close(); err != nil {
		t.Fatal(err)
	}
	wg.Wait()
	assertStreamClosed(t, d)
}
