// Package splintr provides Go bindings to the Rust splintr tokenizer.
package splintr

/*
#cgo CFLAGS: -I${SRCDIR}/native/include
#cgo LDFLAGS: -L${SRCDIR}/build/native -lsplintr_go
#cgo linux LDFLAGS: -ldl -lm -lpthread
#cgo darwin LDFLAGS: -framework Security -framework CoreFoundation -liconv -lresolv
#include "splintr.h"
*/
import "C"

import (
	"errors"
	"fmt"
	"os"
	"runtime"
	"sync"
	"unsafe"
)

// ErrorCode identifies an error status returned by the native library.
type ErrorCode int

const (
	// ErrorInvalidArgument means an argument did not satisfy the ABI contract.
	ErrorInvalidArgument ErrorCode = ErrorCode(C.SPLINTR_ERROR_INVALID_ARGUMENT)
	// ErrorInvalidUTF8 means a text argument was not valid UTF-8.
	ErrorInvalidUTF8 ErrorCode = ErrorCode(C.SPLINTR_ERROR_INVALID_UTF8)
	// ErrorTokenizer means tokenizer parsing or an encode/decode operation failed.
	ErrorTokenizer ErrorCode = ErrorCode(C.SPLINTR_ERROR_TOKENIZER)
	// ErrorPanic means the native library caught a Rust panic.
	ErrorPanic ErrorCode = ErrorCode(C.SPLINTR_ERROR_PANIC)
)

// Error-code aliases.
const (
	CodeInvalidArgument = ErrorInvalidArgument
	CodeInvalidUTF8     = ErrorInvalidUTF8
	CodeTokenizer       = ErrorTokenizer
	CodePanic           = ErrorPanic
)

// Error is an error reported by the native splintr library.
type Error struct {
	Code    ErrorCode
	Message string
}

func (e *Error) Error() string {
	if e == nil {
		return "<nil>"
	}
	if e.Message == "" {
		return fmt.Sprintf("splintr: native error %d", e.Code)
	}
	return "splintr: " + e.Message
}

// ErrClosed is returned when an operation is attempted on a closed, nil, or
// zero-value Tokenizer.
var ErrClosed = errors.New("splintr: tokenizer is closed")

// EncodeSpecialMode controls recognition of special-token spellings.
type EncodeSpecialMode int

const (
	// SpecialOrdinary treats every special-token spelling as ordinary input.
	SpecialOrdinary EncodeSpecialMode = EncodeSpecialMode(C.SPLINTR_SPECIAL_ORDINARY)
	// SpecialAll recognizes every configured special token in the input.
	SpecialAll EncodeSpecialMode = EncodeSpecialMode(C.SPLINTR_SPECIAL_ALL)
)

// DecodeSpecialMode controls how declared special token IDs are decoded.
type DecodeSpecialMode int

const (
	// DecodeSkipSpecial omits declared special tokens from decoded text.
	DecodeSkipSpecial DecodeSpecialMode = DecodeSpecialMode(C.SPLINTR_DECODE_SKIP_SPECIAL)
	// DecodeRenderSpecial renders declared special tokens in decoded text.
	DecodeRenderSpecial DecodeSpecialMode = DecodeSpecialMode(C.SPLINTR_DECODE_RENDER_SPECIAL)
)

type tokenizerState struct {
	mu  sync.RWMutex
	ptr *C.SplintrTokenizer
}

// Tokenizer is a native tokenizer handle. Its zero value is closed.
//
// Tokenizer values may be copied: copies share ownership and closing one makes
// every copy closed. Methods may be called concurrently with each other and
// with Close; Close waits for calls already in progress.
type Tokenizer struct {
	state *tokenizerState
}

func init() {
	const want = uint32(0x00010000)
	if got := uint32(C.splintr_go_version_0_1_0()); got != want {
		panic(fmt.Sprintf("splintr: native ABI version %#x, want %#x", got, want))
	}
}

// FromBytes loads a Hugging Face tokenizer.json document. The data must be
// non-empty. Loading is local and does not retain data.
func FromBytes(data []byte) (*Tokenizer, error) {
	var ptr *C.SplintrTokenizer
	var nativeErr *C.SplintrError
	status := C.splintr_tokenizer_from_json(bytePtr(data), C.size_t(len(data)), &ptr, &nativeErr)
	runtime.KeepAlive(data)
	return finishTokenizer(ptr, status, nativeErr)
}

// FromFile reads and loads a Hugging Face tokenizer.json file.
func FromFile(path string) (*Tokenizer, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}
	return FromBytes(data)
}

// FromPretrained loads a bundled vocabulary by name or alias, without downloads.
func FromPretrained(name string) (*Tokenizer, error) {
	var ptr *C.SplintrTokenizer
	var nativeErr *C.SplintrError
	bytes := []byte(name)
	status := C.splintr_tokenizer_from_pretrained(bytePtr(bytes), C.size_t(len(bytes)), &ptr, &nativeErr)
	runtime.KeepAlive(bytes)
	return finishTokenizer(ptr, status, nativeErr)
}

func finishTokenizer(ptr *C.SplintrTokenizer, status C.SplintrStatus, nativeErr *C.SplintrError) (*Tokenizer, error) {
	if err := nativeError(status, nativeErr); err != nil {
		return nil, err
	}
	if ptr == nil {
		return nil, &Error{Code: CodePanic, Message: "native constructor returned a nil tokenizer"}
	}
	state := &tokenizerState{ptr: ptr}
	runtime.SetFinalizer(state, finalizeTokenizer)
	return &Tokenizer{state: state}, nil
}

func finalizeTokenizer(state *tokenizerState) {
	state.mu.Lock()
	ptr := state.ptr
	state.ptr = nil
	state.mu.Unlock()
	if ptr != nil {
		C.splintr_tokenizer_free(ptr)
	}
}

func (t *Tokenizer) lock() (*tokenizerState, *C.SplintrTokenizer, error) {
	if t == nil || t.state == nil {
		return nil, nil, ErrClosed
	}
	state := t.state
	state.mu.RLock()
	if state.ptr == nil {
		state.mu.RUnlock()
		return nil, nil, ErrClosed
	}
	return state, state.ptr, nil
}

func unlock(state *tokenizerState) {
	state.mu.RUnlock()
	runtime.KeepAlive(state)
}

// Close waits for active calls and releases the shared handle. It is idempotent.
func (t *Tokenizer) Close() error {
	if t == nil || t.state == nil {
		return nil
	}
	state := t.state
	state.mu.Lock()
	ptr := state.ptr
	state.ptr = nil
	state.mu.Unlock()
	if ptr != nil {
		runtime.SetFinalizer(state, nil)
		C.splintr_tokenizer_free(ptr)
	}
	runtime.KeepAlive(state)
	return nil
}

// Encode recognizes all configured special tokens and applies post-processing,
// which may insert special IDs even for empty text.
func (t *Tokenizer) Encode(text string) ([]uint32, error) {
	return t.EncodeWithSpecialMode(text, SpecialAll)
}

// EncodeWithSpecialMode encodes text with explicit special-token recognition.
// The tokenizer.json single-sequence post-processor is applied in both modes.
func (t *Tokenizer) EncodeWithSpecialMode(text string, mode EncodeSpecialMode) ([]uint32, error) {
	state, ptr, err := t.lock()
	if err != nil {
		return nil, err
	}
	defer unlock(state)

	input := []byte(text)
	var out C.SplintrIds
	var nativeErr *C.SplintrError
	status := C.splintr_encode_with_special_mode(
		ptr, bytePtr(input), C.size_t(len(input)), C.SplintrEncodeSpecialMode(mode), &out, &nativeErr,
	)
	runtime.KeepAlive(input)
	if err := nativeError(status, nativeErr); err != nil {
		return nil, err
	}
	defer C.splintr_ids_free(out)
	if out.len == 0 {
		return []uint32{}, nil
	}
	if out.data == nil || uint64(out.len) > uint64(maxInt()) {
		return nil, &Error{Code: CodePanic, Message: "native encoder returned an invalid output buffer"}
	}
	ids := make([]uint32, int(out.len))
	copy(ids, unsafe.Slice((*uint32)(unsafe.Pointer(out.data)), int(out.len)))
	return ids, nil
}

// Decode decodes IDs while skipping declared special tokens. It follows
// splintr's behavior for unknown IDs: declared decoder pipelines may omit
// them, while other decoding paths return an error.
func (t *Tokenizer) Decode(ids []uint32) (string, error) {
	return t.DecodeWithSpecialMode(ids, DecodeSkipSpecial)
}

// DecodeWithSpecialMode decodes IDs while either skipping or rendering
// declared special tokens.
func (t *Tokenizer) DecodeWithSpecialMode(ids []uint32, mode DecodeSpecialMode) (string, error) {
	state, ptr, err := t.lock()
	if err != nil {
		return "", err
	}
	defer unlock(state)

	var out C.SplintrBytes
	var nativeErr *C.SplintrError
	status := C.splintr_decode_with_special_mode(
		ptr, uint32Ptr(ids), C.size_t(len(ids)), C.SplintrDecodeSpecialMode(mode), &out, &nativeErr,
	)
	runtime.KeepAlive(ids)
	if err := nativeError(status, nativeErr); err != nil {
		return "", err
	}
	defer C.splintr_bytes_free(out)
	if out.len == 0 {
		return "", nil
	}
	if out.data == nil || uint64(out.len) > uint64(maxInt()) {
		return "", &Error{Code: CodePanic, Message: "native decoder returned an invalid output buffer"}
	}
	return string(unsafe.Slice((*byte)(unsafe.Pointer(out.data)), int(out.len))), nil
}

// EncodeBatch encodes texts in input order using splintr's native batch processing.
// It has the same special-token and post-processing behavior as Encode. Empty
// batches return an empty slice; any error returns nil rather than partial results.
func (t *Tokenizer) EncodeBatch(texts []string) ([][]uint32, error) {
	state, ptr, err := t.lock()
	if err != nil {
		return nil, err
	}
	defer unlock(state)

	lengths := make([]C.size_t, len(texts))
	total := 0
	for i, text := range texts {
		if len(text) > maxInt()-total {
			return nil, &Error{Code: CodeInvalidArgument, Message: "batch text length is too large"}
		}
		lengths[i] = C.size_t(len(text))
		total += len(text)
	}
	input := make([]byte, 0, total)
	for _, text := range texts {
		input = append(input, text...)
	}
	var out C.SplintrIdsBatch
	var nativeErr *C.SplintrError
	status := C.splintr_encode_batch(
		ptr, bytePtr(input), C.size_t(len(input)),
		unsafe.SliceData(lengths), C.size_t(len(lengths)), &out, &nativeErr,
	)
	runtime.KeepAlive(input)
	runtime.KeepAlive(lengths)
	if err := nativeError(status, nativeErr); err != nil {
		return nil, err
	}
	defer C.splintr_ids_batch_free(out)
	data, sizes, err := batchOutput[uint32](unsafe.Pointer(out.values.data), out.values.len, out.lengths, len(texts))
	if err != nil {
		return nil, err
	}
	ids := make([]uint32, len(data))
	copy(ids, data)
	result := make([][]uint32, len(texts))
	start := 0
	for i, size := range sizes {
		end := start + int(size)
		result[i] = ids[start:end:end]
		start = end
	}
	return result, nil
}

// DecodeBatch decodes token lists in input order using splintr's native batch
// processing. Special tokens and unknown IDs behave as in Decode. Empty batches
// return an empty slice; any error returns nil rather than partial results.
func (t *Tokenizer) DecodeBatch(tokenLists [][]uint32) ([]string, error) {
	state, ptr, err := t.lock()
	if err != nil {
		return nil, err
	}
	defer unlock(state)

	lengths := make([]C.size_t, len(tokenLists))
	total := 0
	for i, ids := range tokenLists {
		if len(ids) > maxInt()/4-total {
			return nil, &Error{Code: CodeInvalidArgument, Message: "batch token count is too large"}
		}
		lengths[i] = C.size_t(len(ids))
		total += len(ids)
	}
	input := make([]uint32, 0, total)
	for _, ids := range tokenLists {
		input = append(input, ids...)
	}
	var out C.SplintrBytesBatch
	var nativeErr *C.SplintrError
	status := C.splintr_decode_batch(
		ptr, uint32Ptr(input), C.size_t(len(input)),
		unsafe.SliceData(lengths), C.size_t(len(lengths)), &out, &nativeErr,
	)
	runtime.KeepAlive(input)
	runtime.KeepAlive(lengths)
	if err := nativeError(status, nativeErr); err != nil {
		return nil, err
	}
	defer C.splintr_bytes_batch_free(out)
	data, sizes, err := batchOutput[byte](unsafe.Pointer(out.values.data), out.values.len, out.lengths, len(tokenLists))
	if err != nil {
		return nil, err
	}
	result := make([]string, len(tokenLists))
	start := 0
	for i, size := range sizes {
		end := start + int(size)
		result[i] = string(data[start:end])
		start = end
	}
	return result, nil
}

// batchOutput validates the borrowed native buffers before constructing slices.
func batchOutput[T byte | uint32](
	data unsafe.Pointer, size C.size_t, lengths C.SplintrLengths, count int,
) ([]T, []C.size_t, error) {
	var zero T
	if lengths.len != C.size_t(count) || (count > 0 && lengths.data == nil) ||
		uint64(size) > uint64(maxInt())/uint64(unsafe.Sizeof(zero)) || (size > 0 && data == nil) {
		return nil, nil, &Error{Code: CodePanic, Message: "native batch returned invalid buffers"}
	}
	sizes := unsafe.Slice(lengths.data, count)
	remaining := uint64(size)
	for _, length := range sizes {
		if uint64(length) > remaining {
			return nil, nil, &Error{Code: CodePanic, Message: "native batch returned an invalid item length"}
		}
		remaining -= uint64(length)
	}
	if remaining != 0 {
		return nil, nil, &Error{Code: CodePanic, Message: "native batch lengths do not match its data"}
	}
	return unsafe.Slice((*T)(data), int(size)), sizes, nil
}

// VocabSize returns the number of entries in the tokenizer vocabulary.
func (t *Tokenizer) VocabSize() (int, error) {
	state, ptr, err := t.lock()
	if err != nil {
		return 0, err
	}
	defer unlock(state)
	var size C.size_t
	var nativeErr *C.SplintrError
	status := C.splintr_tokenizer_vocab_size(ptr, &size, &nativeErr)
	if err := nativeError(status, nativeErr); err != nil {
		return 0, err
	}
	if uint64(size) > uint64(maxInt()) {
		return 0, &Error{Code: CodePanic, Message: "native vocabulary size overflows int"}
	}
	return int(size), nil
}

// Family returns the model family: "BPE", "Unigram", "WordPiece", "Spm", or
// "Unknown".
func (t *Tokenizer) Family() (string, error) {
	state, ptr, err := t.lock()
	if err != nil {
		return "", err
	}
	defer unlock(state)
	var family C.SplintrFamily
	var nativeErr *C.SplintrError
	status := C.splintr_tokenizer_family(ptr, &family, &nativeErr)
	if err := nativeError(status, nativeErr); err != nil {
		return "", err
	}
	switch family {
	case C.SPLINTR_FAMILY_BPE:
		return "BPE", nil
	case C.SPLINTR_FAMILY_UNIGRAM:
		return "Unigram", nil
	case C.SPLINTR_FAMILY_WORDPIECE:
		return "WordPiece", nil
	case C.SPLINTR_FAMILY_SPM:
		return "Spm", nil
	default:
		return "Unknown", nil
	}
}

// EOSTokenID returns the configured end-of-sequence token ID. The boolean is
// false when no EOS token is configured.
func (t *Tokenizer) EOSTokenID() (uint32, bool, error) {
	state, ptr, err := t.lock()
	if err != nil {
		return 0, false, err
	}
	defer unlock(state)
	var found C.uint8_t
	var id C.uint32_t
	var nativeErr *C.SplintrError
	status := C.splintr_tokenizer_eos_id(ptr, &found, &id, &nativeErr)
	if err := nativeError(status, nativeErr); err != nil {
		return 0, false, err
	}
	return uint32(id), found != 0, nil
}

// SpecialTokenID looks up a declared special token by its exact content. The
// boolean is false if token is not declared special.
func (t *Tokenizer) SpecialTokenID(token string) (uint32, bool, error) {
	state, ptr, err := t.lock()
	if err != nil {
		return 0, false, err
	}
	defer unlock(state)
	input := []byte(token)
	var found C.uint8_t
	var id C.uint32_t
	var nativeErr *C.SplintrError
	status := C.splintr_tokenizer_special_token_id(ptr, bytePtr(input), C.size_t(len(input)), &found, &id, &nativeErr)
	runtime.KeepAlive(input)
	if err := nativeError(status, nativeErr); err != nil {
		return 0, false, err
	}
	return uint32(id), found != 0, nil
}

func bytePtr(data []byte) *C.uint8_t {
	return (*C.uint8_t)(unsafe.Pointer(unsafe.SliceData(data)))
}

func uint32Ptr(ids []uint32) *C.uint32_t {
	return (*C.uint32_t)(unsafe.Pointer(unsafe.SliceData(ids)))
}

func nativeError(status C.SplintrStatus, nativeErr *C.SplintrError) error {
	if nativeErr != nil {
		defer C.splintr_error_free(nativeErr)
	}
	if status == C.SPLINTR_OK {
		return nil
	}

	message := ""
	if nativeErr != nil {
		var data *C.uint8_t
		var length C.size_t
		messageStatus := C.splintr_error_message(nativeErr, &data, &length)
		if messageStatus == C.SPLINTR_OK && data != nil && uint64(length) <= uint64(maxInt()) {
			message = string(unsafe.Slice((*byte)(unsafe.Pointer(data)), int(length)))
		}
	}
	return &Error{Code: ErrorCode(status), Message: message}
}

func maxInt() int {
	return int(^uint(0) >> 1)
}
