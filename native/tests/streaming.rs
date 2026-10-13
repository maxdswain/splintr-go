use super::*;

const BYTELEVEL: &[u8] = include_bytes!("../../tests/test_data/bytelevel.json");

fn assert_send<T: Send>() {}

#[test]
fn stream_can_migrate_threads() {
    assert_send::<StreamingDecoder>();
    assert_send::<SplintrStreamingDecoder>();
}

unsafe fn bytelevel() -> *mut SplintrTokenizer {
    let mut tokenizer = ptr::null_mut();
    assert_eq!(
        unsafe { splintr_tokenizer_from_json(BYTELEVEL.as_ptr(), BYTELEVEL.len(), &mut tokenizer, ptr::null_mut()) },
        SPLINTR_OK
    );
    tokenizer
}

unsafe fn stream(tokenizer: *const SplintrTokenizer, mode: i32) -> *mut SplintrStreamingDecoder {
    let mut decoder = ptr::null_mut();
    assert_eq!(
        unsafe { splintr_streaming_decoder_new(tokenizer, mode, &mut decoder, ptr::null_mut()) },
        SPLINTR_OK
    );
    decoder
}

unsafe fn add(decoder: *mut SplintrStreamingDecoder, ids: &[u32], lossy: u8) -> String {
    let mut text = SplintrBytes {
        data: ptr::null_mut(),
        len: 0,
        capacity: 0,
    };
    assert_eq!(
        unsafe {
            splintr_streaming_decoder_add_tokens(decoder, ids.as_ptr(), ids.len(), lossy, &mut text, ptr::null_mut())
        },
        SPLINTR_OK
    );
    let value = if text.len == 0 {
        String::new()
    } else {
        // SAFETY: the returned buffer holds exactly len initialized bytes.
        String::from_utf8(unsafe { slice::from_raw_parts(text.data, text.len) }.to_vec()).unwrap()
    };
    unsafe { splintr_bytes_free(text) };
    value
}

unsafe fn flush(decoder: *mut SplintrStreamingDecoder) -> String {
    let mut text = SplintrBytes {
        data: ptr::null_mut(),
        len: 0,
        capacity: 0,
    };
    assert_eq!(
        unsafe { splintr_streaming_decoder_flush(decoder, &mut text, ptr::null_mut()) },
        SPLINTR_OK
    );
    let value = if text.len == 0 {
        String::new()
    } else {
        // SAFETY: the returned buffer holds exactly len initialized bytes.
        String::from_utf8(unsafe { slice::from_raw_parts(text.data, text.len) }.to_vec()).unwrap()
    };
    unsafe { splintr_bytes_free(text) };
    value
}

unsafe fn pending(decoder: *const SplintrStreamingDecoder) -> (u8, usize) {
    let mut flag = 255;
    let mut bytes = usize::MAX;
    assert_eq!(
        unsafe { splintr_streaming_decoder_has_pending(decoder, &mut flag, ptr::null_mut()) },
        SPLINTR_OK
    );
    assert_eq!(
        unsafe { splintr_streaming_decoder_pending_bytes(decoder, &mut bytes, ptr::null_mut()) },
        SPLINTR_OK
    );
    (flag, bytes)
}

#[test]
fn validates_constructor_and_all_null_handle_operations() {
    unsafe {
        let tokenizer = super::tests::load();
        let mut decoder = ptr::dangling_mut::<SplintrStreamingDecoder>();
        let mut error = ptr::dangling_mut::<SplintrError>();
        assert_eq!(
            splintr_streaming_decoder_new(tokenizer, 99, &mut decoder, &mut error),
            SPLINTR_ERROR_INVALID_ARGUMENT
        );
        assert!(decoder.is_null());
        assert!(!error.is_null());
        splintr_error_free(error);
        error = ptr::null_mut();
        assert_eq!(
            splintr_streaming_decoder_new(ptr::null(), 0, &mut decoder, &mut error),
            SPLINTR_ERROR_INVALID_ARGUMENT
        );
        assert!(decoder.is_null());
        splintr_error_free(error);
        assert_eq!(
            splintr_streaming_decoder_new(tokenizer, 0, ptr::null_mut(), ptr::null_mut()),
            SPLINTR_ERROR_INVALID_ARGUMENT
        );
        let mut uninitialized = std::mem::MaybeUninit::<*mut SplintrStreamingDecoder>::uninit();
        assert_eq!(
            splintr_streaming_decoder_new(tokenizer, 1, uninitialized.as_mut_ptr(), ptr::null_mut()),
            SPLINTR_OK
        );
        splintr_streaming_decoder_free(uninitialized.assume_init());
        let valid = stream(tokenizer, 0);
        let mut text = std::mem::MaybeUninit::<SplintrBytes>::uninit();
        error = ptr::dangling_mut();
        assert_eq!(
            splintr_streaming_decoder_add_tokens(ptr::null_mut(), ptr::null(), 0, 0, text.as_mut_ptr(), &mut error),
            SPLINTR_ERROR_INVALID_ARGUMENT
        );
        let empty = text.assume_init_ref();
        assert!(empty.data.is_null());
        assert_eq!((empty.len, empty.capacity), (0, 0));
        assert!(!error.is_null());
        splintr_error_free(error);
        assert_eq!(
            splintr_streaming_decoder_add_tokens(valid, ptr::null(), 0, 0, ptr::null_mut(), ptr::null_mut()),
            SPLINTR_ERROR_INVALID_ARGUMENT
        );
        error = ptr::dangling_mut();
        assert_eq!(
            splintr_streaming_decoder_flush(ptr::null_mut(), text.as_mut_ptr(), &mut error),
            SPLINTR_ERROR_INVALID_ARGUMENT
        );
        assert!(text.assume_init_ref().data.is_null());
        splintr_error_free(error);
        assert_eq!(
            splintr_streaming_decoder_flush(valid, ptr::null_mut(), ptr::null_mut()),
            SPLINTR_ERROR_INVALID_ARGUMENT
        );
        error = ptr::dangling_mut();
        assert_eq!(
            splintr_streaming_decoder_reset(ptr::null_mut(), &mut error),
            SPLINTR_ERROR_INVALID_ARGUMENT
        );
        splintr_error_free(error);
        let mut flag = std::mem::MaybeUninit::<u8>::uninit();
        error = ptr::dangling_mut();
        assert_eq!(
            splintr_streaming_decoder_has_pending(ptr::null(), flag.as_mut_ptr(), &mut error),
            SPLINTR_ERROR_INVALID_ARGUMENT
        );
        assert_eq!(flag.assume_init(), 0);
        splintr_error_free(error);
        assert_eq!(
            splintr_streaming_decoder_has_pending(valid, ptr::null_mut(), ptr::null_mut()),
            SPLINTR_ERROR_INVALID_ARGUMENT
        );
        let mut bytes = std::mem::MaybeUninit::<usize>::uninit();
        error = ptr::dangling_mut();
        assert_eq!(
            splintr_streaming_decoder_pending_bytes(ptr::null(), bytes.as_mut_ptr(), &mut error),
            SPLINTR_ERROR_INVALID_ARGUMENT
        );
        assert_eq!(bytes.assume_init(), 0);
        splintr_error_free(error);
        assert_eq!(
            splintr_streaming_decoder_pending_bytes(valid, ptr::null_mut(), ptr::null_mut()),
            SPLINTR_ERROR_INVALID_ARGUMENT
        );
        splintr_streaming_decoder_free(ptr::null_mut());
        splintr_streaming_decoder_free(valid);
        splintr_tokenizer_free(tokenizer);
    }
}

#[test]
fn rejects_invalid_inputs_without_touching_state_or_ids() {
    unsafe {
        let tokenizer = bytelevel();
        let decoder = stream(tokenizer, 0);
        assert_eq!(add(decoder, &[2], 0), "");
        let before = pending(decoder);
        let id = 3u32;
        let mut output = std::mem::MaybeUninit::<SplintrBytes>::uninit();
        let mut error = ptr::null_mut();
        let bad_len = (isize::MAX as usize) / std::mem::size_of::<u32>() + 1;
        assert_eq!(
            splintr_streaming_decoder_add_tokens(decoder, &id, bad_len, 0, output.as_mut_ptr(), &mut error),
            SPLINTR_ERROR_INVALID_ARGUMENT
        );
        assert!(output.assume_init_ref().data.is_null());
        splintr_error_free(error);
        assert_eq!(pending(decoder), before);
        error = ptr::null_mut();
        assert_eq!(
            splintr_streaming_decoder_add_tokens(decoder, ptr::null(), 1, 0, output.as_mut_ptr(), &mut error),
            SPLINTR_ERROR_INVALID_ARGUMENT
        );
        splintr_error_free(error);
        error = ptr::null_mut();
        assert_eq!(
            splintr_streaming_decoder_add_tokens(decoder, &id, 1, 2, output.as_mut_ptr(), &mut error),
            SPLINTR_ERROR_INVALID_ARGUMENT
        );
        splintr_error_free(error);
        assert_eq!(pending(decoder), before);
        assert_eq!(add(decoder, &[3], 0), "é");
        splintr_streaming_decoder_free(decoder);
        splintr_tokenizer_free(tokenizer);
    }
}

#[test]
fn utf8_nul_flush_reset_independence_and_parent_lifetime() {
    unsafe {
        let tokenizer = bytelevel();
        let first = stream(tokenizer, 0);
        let second = stream(tokenizer, 1);
        splintr_tokenizer_free(tokenizer);
        assert_eq!(add(first, &[2], 0), "");
        assert_eq!(pending(first), (1, 1));
        assert_eq!(pending(second), (0, 0));
        assert_eq!(add(first, &[3, 1], 0), "é\0");
        assert_eq!(add(first, &[4, 5], 0), "");
        assert_eq!(pending(first), (1, 2));
        assert_eq!(add(first, &[6, 7], 0), "🙂");
        assert_eq!(flush(first), "");
        assert_eq!(add(first, &[4], 0), "");
        assert_eq!(flush(first), "�");
        assert_eq!(pending(first), (0, 0));
        assert_eq!(add(first, &[0], 0), "a"); // flush does not close stream
        assert_eq!(add(second, &[2], 0), "");
        assert_eq!(splintr_streaming_decoder_reset(second, ptr::null_mut()), SPLINTR_OK);
        assert_eq!(pending(second), (0, 0));
        assert_eq!(add(second, &[0], 0), "a");
        assert_eq!(add(first, &[], 0), "");
        splintr_streaming_decoder_free(second);
        splintr_streaming_decoder_free(first);
    }
}

#[test]
fn strict_unknown_keeps_accepted_prefix_and_lossy_skips_unknown() {
    unsafe {
        let tokenizer = bytelevel();
        let decoder = stream(tokenizer, 0);
        let ids = [2, 999, 3];
        let mut text = std::mem::MaybeUninit::<SplintrBytes>::uninit();
        let mut error = ptr::null_mut();
        assert_eq!(
            splintr_streaming_decoder_add_tokens(decoder, ids.as_ptr(), ids.len(), 0, text.as_mut_ptr(), &mut error),
            SPLINTR_ERROR_TOKENIZER
        );
        assert!(text.assume_init_ref().data.is_null());
        assert!(!error.is_null());
        splintr_error_free(error);
        assert_eq!(pending(decoder), (1, 1));
        assert_eq!(add(decoder, &[3], 0), "é");
        let prefix = [0, 999, 0];
        error = ptr::null_mut();
        assert_eq!(
            splintr_streaming_decoder_add_tokens(
                decoder,
                prefix.as_ptr(),
                prefix.len(),
                0,
                text.as_mut_ptr(),
                &mut error
            ),
            SPLINTR_ERROR_TOKENIZER
        );
        assert!(text.assume_init_ref().data.is_null());
        splintr_error_free(error);
        assert_eq!(flush(decoder), "a"); // strict errors retain accepted text
        assert_eq!(add(decoder, &[0, 999, 0], 1), "aa");
        assert_eq!(add(decoder, &[2], 0), "");
        assert_eq!(splintr_streaming_decoder_reset(decoder, ptr::null_mut()), SPLINTR_OK);
        assert_eq!(add(decoder, &[0], 0), "a");
        splintr_streaming_decoder_free(decoder);
        splintr_tokenizer_free(tokenizer);
    }
}

#[test]
fn special_mode_skip_and_render() {
    unsafe {
        let tokenizer = super::tests::load();
        let skip = stream(tokenizer, 0);
        let render = stream(tokenizer, 1);
        let skipped = add(skip, &[1, 2], 0) + &flush(skip);
        let rendered = add(render, &[1, 2], 0) + &flush(render);
        assert_eq!(skipped, "hello");
        assert!(rendered.contains("[SEP]"));
        assert!(rendered.contains("hello"));
        splintr_streaming_decoder_free(skip);
        splintr_streaming_decoder_free(render);
        splintr_tokenizer_free(tokenizer);
    }
}

#[test]
fn rejects_unstreamable_declared_pipeline() {
    let fixture = str::from_utf8(BYTELEVEL).unwrap().replace("\r\n", "\n");
    for newline in ["\n", "\r\n"] {
        let source = fixture.replace('\n', newline);
        let json = source.replace(
            "\"decoder\": {\"type\": \"ByteLevel\"}",
            "\"decoder\": {\"type\": \"BPEDecoder\", \"suffix\": \"</w>\"}",
        );
        assert_ne!(json, source, "replacement failed with {newline:?} line endings");
        unsafe {
            let mut tokenizer = ptr::null_mut();
            assert_eq!(
                splintr_tokenizer_from_json(json.as_ptr(), json.len(), &mut tokenizer, ptr::null_mut()),
                SPLINTR_OK
            );
            let mut decoder = ptr::dangling_mut::<SplintrStreamingDecoder>();
            let mut error = ptr::null_mut();
            assert_eq!(
                splintr_streaming_decoder_new(tokenizer, 0, &mut decoder, &mut error),
                SPLINTR_ERROR_TOKENIZER
            );
            assert!(decoder.is_null());
            assert!(!error.is_null());
            splintr_error_free(error);
            splintr_tokenizer_free(tokenizer);
        }
    }
}
