use super::*;

mod batch;

const WORDPIECE: &[u8] = br###"{
  "version":"1.0",
  "added_tokens":[
    {"id":0,"content":"[UNK]","single_word":false,"lstrip":false,"rstrip":false,"normalized":false,"special":true},
    {"id":1,"content":"[SEP]","single_word":false,"lstrip":false,"rstrip":false,"normalized":false,"special":true}
  ],
  "pre_tokenizer":{"type":"BertPreTokenizer"},
  "model":{"type":"WordPiece","unk_token":"[UNK]","continuing_subword_prefix":"##","max_input_chars_per_word":100,
    "vocab":{"[UNK]":0,"[SEP]":1,"hello":2,"world":3,"token":4,"##izer":5}}
}"###;

unsafe fn load() -> *mut SplintrTokenizer {
    let mut tokenizer = ptr::null_mut();
    let mut error = ptr::null_mut();
    assert_eq!(
        unsafe { splintr_tokenizer_from_json(WORDPIECE.as_ptr(), WORDPIECE.len(), &mut tokenizer, &mut error,) },
        SPLINTR_OK
    );
    assert!(error.is_null());
    tokenizer
}

#[test]
fn rejects_empty_and_null_constructor_inputs() {
    unsafe {
        let mut tokenizer = ptr::dangling_mut::<SplintrTokenizer>();
        let mut error = ptr::null_mut();
        assert_eq!(
            splintr_tokenizer_from_json(ptr::null(), 0, &mut tokenizer, &mut error),
            SPLINTR_ERROR_INVALID_ARGUMENT
        );
        assert!(tokenizer.is_null());
        assert!(!error.is_null());
        splintr_error_free(error);

        assert_eq!(
            splintr_tokenizer_from_json(ptr::null(), 1, &mut tokenizer, ptr::null_mut()),
            SPLINTR_ERROR_INVALID_ARGUMENT
        );
        assert_eq!(
            splintr_tokenizer_from_json(WORDPIECE.as_ptr(), WORDPIECE.len(), ptr::null_mut(), ptr::null_mut()),
            SPLINTR_ERROR_INVALID_ARGUMENT
        );
    }
}

#[test]
fn rejects_excessive_input_lengths_without_reading_memory() {
    unsafe {
        let byte = 0u8;
        let mut tokenizer = ptr::null_mut();
        let mut error = ptr::null_mut();
        assert_eq!(
            splintr_tokenizer_from_json(&byte, (isize::MAX as usize) + 1, &mut tokenizer, &mut error,),
            SPLINTR_ERROR_INVALID_ARGUMENT
        );
        assert!(tokenizer.is_null());
        assert!(!error.is_null());
        splintr_error_free(error);

        let tokenizer = load();
        let id = 2u32;
        let mut bytes = SplintrBytes {
            data: ptr::null_mut(),
            len: 0,
            capacity: 0,
        };
        error = ptr::null_mut();
        assert_eq!(
            splintr_decode(
                tokenizer,
                &id,
                (isize::MAX as usize) / std::mem::size_of::<u32>() + 1,
                &mut bytes,
                &mut error,
            ),
            SPLINTR_ERROR_INVALID_ARGUMENT
        );
        assert!(bytes.data.is_null());
        assert!(!error.is_null());
        splintr_error_free(error);
        splintr_tokenizer_free(tokenizer);
    }
}

#[test]
fn ffi_contains_panics_and_returns_owned_error() {
    unsafe {
        let mut error = ptr::dangling_mut::<SplintrError>();
        assert_eq!(
            ffi(&mut error, || -> Result<(), AbiError> {
                panic!("deliberate test panic")
            }),
            SPLINTR_ERROR_PANIC
        );
        assert!(!error.is_null());

        let mut message = ptr::null();
        let mut len = 0;
        assert_eq!(splintr_error_message(error, &mut message, &mut len), SPLINTR_OK);
        let message = str::from_utf8(slice::from_raw_parts(message, len)).unwrap();
        assert!(message.contains("deliberate test panic"));
        splintr_error_free(error);
    }
}

#[test]
fn malformed_json_returns_owned_error() {
    unsafe {
        let malformed = b"{not json";
        let mut tokenizer = ptr::null_mut();
        let mut error = ptr::null_mut();
        assert_eq!(
            splintr_tokenizer_from_json(malformed.as_ptr(), malformed.len(), &mut tokenizer, &mut error),
            SPLINTR_ERROR_TOKENIZER
        );
        let mut message = ptr::null();
        let mut len = 0;
        assert_eq!(splintr_error_message(error, &mut message, &mut len), SPLINTR_OK);
        assert!(len > 0);
        assert!(!message.is_null());
        splintr_error_free(error);
    }
}

#[test]
fn wordpiece_encode_decode_and_metadata() {
    unsafe {
        let tokenizer = load();
        let text = b"hello tokenizer";
        let mut ids = SplintrIds {
            data: ptr::null_mut(),
            len: 0,
            capacity: 0,
        };
        assert_eq!(
            splintr_encode(tokenizer, text.as_ptr(), text.len(), &mut ids, ptr::null_mut()),
            SPLINTR_OK
        );
        assert_eq!(slice::from_raw_parts(ids.data, ids.len), &[2, 4, 5]);

        let mut decoded = SplintrBytes {
            data: ptr::null_mut(),
            len: 0,
            capacity: 0,
        };
        assert_eq!(
            splintr_decode(tokenizer, ids.data, ids.len, &mut decoded, ptr::null_mut()),
            SPLINTR_OK
        );
        assert_eq!(slice::from_raw_parts(decoded.data, decoded.len), b"hello tokenizer");

        let mut size = 0;
        let mut family = 0;
        assert_eq!(
            splintr_tokenizer_vocab_size(tokenizer, &mut size, ptr::null_mut()),
            SPLINTR_OK
        );
        assert_eq!(size, 6);
        assert_eq!(
            splintr_tokenizer_family(tokenizer, &mut family, ptr::null_mut()),
            SPLINTR_OK
        );
        assert_eq!(family, 3);

        let mut found = 0;
        let mut id = 99;
        assert_eq!(
            splintr_tokenizer_special_token_id(tokenizer, b"[SEP]".as_ptr(), 5, &mut found, &mut id, ptr::null_mut()),
            SPLINTR_OK
        );
        assert_eq!((found, id), (1, 1));

        splintr_bytes_free(decoded);
        splintr_ids_free(ids);
        splintr_tokenizer_free(tokenizer);
    }
}

#[test]
fn declared_decoder_ignores_unknown_ids() {
    unsafe {
        let json = str::from_utf8(WORDPIECE).unwrap().replace(
            "\"pre_tokenizer\"",
            "\"decoder\":{\"type\":\"WordPiece\",\"prefix\":\"##\",\"cleanup\":true},\"pre_tokenizer\"",
        );
        let mut tokenizer = ptr::null_mut();
        assert_eq!(
            splintr_tokenizer_from_json(json.as_ptr(), json.len(), &mut tokenizer, ptr::null_mut(),),
            SPLINTR_OK
        );

        let ids = [2, 999, 3];
        let mut bytes = SplintrBytes {
            data: ptr::null_mut(),
            len: 0,
            capacity: 0,
        };
        assert_eq!(
            splintr_decode(tokenizer, ids.as_ptr(), ids.len(), &mut bytes, ptr::null_mut(),),
            SPLINTR_OK
        );
        assert_eq!(slice::from_raw_parts(bytes.data, bytes.len), b"hello world");
        splintr_bytes_free(bytes);
        splintr_tokenizer_free(tokenizer);
    }
}

#[test]
fn validates_operation_inputs_and_upstream_errors() {
    unsafe {
        let tokenizer = load();
        let mut ids = SplintrIds {
            data: ptr::null_mut(),
            len: 0,
            capacity: 0,
        };
        assert_eq!(
            splintr_encode(tokenizer, ptr::null(), 0, &mut ids, ptr::null_mut()),
            SPLINTR_OK
        );
        splintr_ids_free(ids);
        ids = SplintrIds {
            data: ptr::null_mut(),
            len: 0,
            capacity: 0,
        };

        let bad_utf8 = [0xff];
        assert_eq!(
            splintr_encode(tokenizer, bad_utf8.as_ptr(), 1, &mut ids, ptr::null_mut()),
            SPLINTR_ERROR_INVALID_UTF8
        );
        assert_eq!(
            splintr_encode(ptr::null(), ptr::null(), 0, &mut ids, ptr::null_mut()),
            SPLINTR_ERROR_INVALID_ARGUMENT
        );

        let invalid_id = [999u32];
        let mut bytes = SplintrBytes {
            data: ptr::null_mut(),
            len: 0,
            capacity: 0,
        };
        assert_eq!(
            splintr_decode(tokenizer, invalid_id.as_ptr(), 1, &mut bytes, ptr::null_mut()),
            SPLINTR_ERROR_TOKENIZER
        );
        assert_eq!(
            splintr_decode(tokenizer, ptr::null(), 1, &mut bytes, ptr::null_mut()),
            SPLINTR_ERROR_INVALID_ARGUMENT
        );
        splintr_tokenizer_free(tokenizer);
    }
}
