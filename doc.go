// Package splintr provides local text tokenization through the Rust splintr library.
// It supports whole-text encoding and decoding, parallel batches, and streaming decoding.
// See [installation instructions] for cgo and native-library setup.
//
// Start with [FromPretrained] for a bundled vocabulary or [FromFile] for a Hugging Face
// tokenizer.json. Reuse the resulting [Tokenizer] and call [Tokenizer.Close] when done.
//
// # Getting started
//
// Default native builds include 23 vocabulary variants, including cl100k_base,
// llama3, qwen3, and deepseek_v3. Loading a bundled vocabulary needs no downloads:
//
//	tok, err := splintr.FromPretrained("cl100k_base")
//	if err != nil {
//		log.Fatal(err)
//	}
//	defer tok.Close()
//
//	ids, err := tok.Encode("Hello, world!")
//	if err != nil {
//		log.Fatal(err)
//	}
//	fmt.Println(ids) // [9906 11 1917 0]
//
// Use [Tokenizer.Decode] to turn IDs back into text, or [FromBytes] to load a JSON
// document already in memory. Token IDs belong to the vocabulary that produced them;
// use the same tokenizer to decode them. Normalization and special-token handling mean
// decoding is not always an exact inverse of encoding. See [pretrained vocabularies]
// for supported names and model-specific behavior.
//
// # Batch processing
//
// [Tokenizer.EncodeBatch] and [Tokenizer.DecodeBatch] preserve input order and use
// the same defaults as their single-input counterparts. Splintr runs larger batches
// in parallel. Empty batches return empty slices; errors return no partial results.
//
// # Streaming decoding
//
// Create a [StreamingDecoder] with [Tokenizer.StreamingDecoder], then feed IDs with
// [StreamingDecoder.AddToken] or [StreamingDecoder.AddTokens]. Concatenate the returned
// fragments, which may be empty while UTF-8 characters or decoder operations are pending.
// Call [StreamingDecoder.Flush] at the end to emit remaining text; incomplete UTF-8 is
// replaced with U+FFFD. Flush does not close or reset the stream.
//
// Unknown IDs return an error; any prefix accepted by that call stays buffered.
// [StreamingDecoder.AddTokensLossy] skips unknown IDs instead. [StreamingDecoder.Reset]
// discards pending text and starts a new sequence. Some tokenizer.json decoder pipelines
// cannot stream and fail at construction. Encoding requires complete input text;
// there is no streaming encoder.
//
// # Special tokens
//
// [Tokenizer.Encode] recognizes configured special-token spellings using [SpecialAll].
// For untrusted text, use [Tokenizer.EncodeWithSpecialMode] with [SpecialOrdinary].
// Both modes still apply the tokenizer's post-processor, which may insert special IDs.
//
// [Tokenizer.Decode] and [Tokenizer.StreamingDecoder] skip special tokens by default.
// Use their WithSpecialMode variants with [DecodeRenderSpecial] to include them.
// Ordinary decoding may omit unknown IDs in declared decoder pipelines; strict
// streaming adds reject them. [Tokenizer.SpecialTokenID] and [Tokenizer.EOSTokenID]
// provide special-token metadata.
//
// # Ownership and concurrency
//
// Tokenizer methods support concurrent calls. Stream methods serialize access to one
// cursor; callers must coordinate token order when sharing a stream. Streams own their
// state and remain usable after their tokenizer closes.
//
// Copies of either handle share ownership, so closing one closes all its copies.
// Close waits for active calls and is idempotent. [StreamingDecoder.Close] discards
// pending text; flush first if it is needed. Returned slices and strings are Go-owned
// and remain valid after closing the handle.
//
// # Errors
//
// Operations on nil, zero-value, or closed handles return [ErrClosed]; Close remains
// safe to call. Use [errors.Is] to check this sentinel and [errors.AsType] to inspect a
// native [Error] and its [ErrorCode]. [FromFile] also returns filesystem errors.
//
// # Native library
//
// Building requires Go 1.26+, cgo, a C compiler, and a matching native static library.
// The go get command downloads the Go module, not the native library. See
// [installation instructions] for prebuilt archives or [source builds] for Rust builds.
// Bundled vocabularies have separate [license terms].
//
// [pretrained vocabularies]: https://github.com/maxdswain/splintr-go/blob/main/docs/pretrained.md
// [installation instructions]: https://github.com/maxdswain/splintr-go#installation
// [source builds]: https://github.com/maxdswain/splintr-go/blob/main/docs/development.md
// [license terms]: https://github.com/maxdswain/splintr-go/blob/main/LICENSE-OTHERS
package splintr
