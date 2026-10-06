#ifndef SPLINTR_GO_H
#define SPLINTR_GO_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Opaque, heap-owned objects. */
typedef struct SplintrTokenizer SplintrTokenizer;
typedef struct SplintrError SplintrError;

typedef enum SplintrStatus {
    SPLINTR_OK = 0,
    SPLINTR_ERROR_INVALID_ARGUMENT = 1,
    SPLINTR_ERROR_INVALID_UTF8 = 2,
    SPLINTR_ERROR_TOKENIZER = 3,
    SPLINTR_ERROR_PANIC = 4
} SplintrStatus;

typedef enum SplintrEncodeSpecialMode {
    SPLINTR_SPECIAL_ORDINARY = 0, /* Treat special-token spellings as plain text. */
    SPLINTR_SPECIAL_ALL = 1       /* Recognize configured special tokens. */
} SplintrEncodeSpecialMode;

typedef enum SplintrDecodeSpecialMode {
    SPLINTR_DECODE_SKIP_SPECIAL = 0,
    SPLINTR_DECODE_RENDER_SPECIAL = 1
} SplintrDecodeSpecialMode;

typedef enum SplintrFamily {
    SPLINTR_FAMILY_UNKNOWN = 0,
    SPLINTR_FAMILY_BPE = 1,
    SPLINTR_FAMILY_UNIGRAM = 2,
    SPLINTR_FAMILY_WORDPIECE = 3,
    SPLINTR_FAMILY_SPM = 4
} SplintrFamily;

/*
 * Rust-owned buffers: read len elements, then pass the unchanged triple once
 * to its matching free function. Empty buffers are {NULL, 0, 0}.
 */
typedef struct SplintrBytes {
    uint8_t *data;
    size_t len;
    size_t capacity;
} SplintrBytes;

typedef struct SplintrIds {
    uint32_t *data;
    size_t len;
    size_t capacity;
} SplintrIds;

typedef struct SplintrLengths {
    size_t *data;
    size_t len;
    size_t capacity;
} SplintrLengths;

typedef struct SplintrIdsBatch {
    SplintrIds values;
    SplintrLengths lengths;
} SplintrIdsBatch;

typedef struct SplintrBytesBatch {
    SplintrBytes values;
    SplintrLengths lengths;
} SplintrBytesBatch;

/* Link/version probe. Returns 0x00010000 for ABI 0.1.0. */
uint32_t splintr_go_version_0_1_0(void);

/*
 * Constructors return one owned handle. JSON/name must be non-empty.
 * JSON uses Hugging Face's tokenizer.json format. Pretrained names require
 * their vocabulary feature; the default build includes all vocabularies.
 */
SplintrStatus splintr_tokenizer_from_json(
    const uint8_t *json, size_t json_len,
    SplintrTokenizer **out_tokenizer, SplintrError **out_error);
SplintrStatus splintr_tokenizer_from_pretrained(
    const uint8_t *name, size_t name_len,
    SplintrTokenizer **out_tokenizer, SplintrError **out_error);

/*
 * Encode defaults to ALL. Both modes apply the single-sequence post-processor.
 * Empty text is valid, including (NULL, 0).
 */
SplintrStatus splintr_encode(
    const SplintrTokenizer *tokenizer,
    const uint8_t *text, size_t text_len,
    SplintrIds *out_ids, SplintrError **out_error);
SplintrStatus splintr_encode_with_special_mode(
    const SplintrTokenizer *tokenizer,
    const uint8_t *text, size_t text_len,
    SplintrEncodeSpecialMode mode,
    SplintrIds *out_ids, SplintrError **out_error);

/*
 * Decode defaults to SKIP_SPECIAL; (NULL, 0) is valid. Declared decoder
 * pipelines may omit unknown IDs; other paths may return a tokenizer error.
 */
SplintrStatus splintr_decode(
    const SplintrTokenizer *tokenizer,
    const uint32_t *ids, size_t ids_len,
    SplintrBytes *out_text, SplintrError **out_error);
SplintrStatus splintr_decode_with_special_mode(
    const SplintrTokenizer *tokenizer,
    const uint32_t *ids, size_t ids_len,
    SplintrDecodeSpecialMode mode,
    SplintrBytes *out_text, SplintrError **out_error);

/*
 * Batch inputs are contiguous concatenated payloads, partitioned by exactly
 * count lengths whose sum must equal data_len. Encode validates UTF-8 per item;
 * decode lengths count uint32_t IDs. (NULL, 0, NULL, 0) and empty items are
 * valid. Outputs concatenate the results in input order; lengths records each
 * result size (IDs for encode, UTF-8 bytes for decode). These use the normal
 * encode and decode defaults; there are no batch special-mode variants.
 * Free only the complete returned batch with its matching batch free function,
 * never its individual fields. On failure all output fields are zeroed.
 */
SplintrStatus splintr_encode_batch(
    const SplintrTokenizer *tokenizer,
    const uint8_t *data, size_t data_len,
    const size_t *lengths, size_t count,
    SplintrIdsBatch *out, SplintrError **out_error);
SplintrStatus splintr_decode_batch(
    const SplintrTokenizer *tokenizer,
    const uint32_t *data, size_t data_len,
    const size_t *lengths, size_t count,
    SplintrBytesBatch *out, SplintrError **out_error);

SplintrStatus splintr_tokenizer_vocab_size(
    const SplintrTokenizer *tokenizer,
    size_t *out_size, SplintrError **out_error);
SplintrStatus splintr_tokenizer_family(
    const SplintrTokenizer *tokenizer,
    SplintrFamily *out_family, SplintrError **out_error);

/*
 * Missing values return OK with found=0, id=0; present values have found=1.
 * Special-token lookup uses non-empty UTF-8 token content, e.g. "[SEP]".
 */
SplintrStatus splintr_tokenizer_eos_id(
    const SplintrTokenizer *tokenizer,
    uint8_t *out_found, uint32_t *out_id, SplintrError **out_error);
SplintrStatus splintr_tokenizer_special_token_id(
    const SplintrTokenizer *tokenizer,
    const uint8_t *token, size_t token_len,
    uint8_t *out_found, uint32_t *out_id, SplintrError **out_error);

/* NULL is accepted by all object/buffer free functions where representable. */
void splintr_tokenizer_free(SplintrTokenizer *tokenizer);
void splintr_bytes_free(SplintrBytes buffer);
void splintr_ids_free(SplintrIds buffer);
void splintr_ids_batch_free(SplintrIdsBatch buffer);
void splintr_bytes_batch_free(SplintrBytesBatch buffer);

/*
 * out_error is optional and cleared before each operation. On failure it
 * receives an owned error; free it once with splintr_error_free. The message
 * accessor returns borrowed UTF-8 bytes, valid until the error is freed.
 */
SplintrStatus splintr_error_message(
    const SplintrError *error,
    const uint8_t **out_message, size_t *out_len);
void splintr_error_free(SplintrError *error);

/*
 * Contract:
 * - Inputs are length-delimited, not NUL-terminated, and live through the call.
 *   Nonzero lengths require valid input storage; text/name bytes must be UTF-8.
 * - Outputs (including out_error) must be exclusive writable storage, disjoint
 *   from inputs, live handles/errors and every other output for the whole call.
 *   They may be uninitialized; free previous contents before reusing a slot.
 * - Fallible operations with valid required outputs zero those outputs before
 *   further validation. The borrowed error-message accessor is an exception.
 * - Concurrent tokenizer calls are allowed; freeing must wait for all calls.
 *   Double-free, use-after-free and modifying owned buffer triples are invalid.
 * - Panics are caught and reported as PANIC. Errors are returned per call.
 */

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* SPLINTR_GO_H */
