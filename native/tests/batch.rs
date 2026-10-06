use super::*;

unsafe fn ids_batch_values(batch: &SplintrIdsBatch) -> Vec<Vec<u32>> {
    let lengths = unsafe { slice::from_raw_parts(batch.lengths.data, batch.lengths.len) };
    let values = if batch.values.len == 0 {
        &[][..]
    } else {
        unsafe { slice::from_raw_parts(batch.values.data, batch.values.len) }
    };
    let mut pos = 0;
    lengths
        .iter()
        .map(|&len| {
            let item = values[pos..pos + len].to_vec();
            pos += len;
            item
        })
        .collect()
}

unsafe fn bytes_batch_values(batch: &SplintrBytesBatch) -> Vec<Vec<u8>> {
    let lengths = unsafe { slice::from_raw_parts(batch.lengths.data, batch.lengths.len) };
    let values = if batch.values.len == 0 {
        &[][..]
    } else {
        unsafe { slice::from_raw_parts(batch.values.data, batch.values.len) }
    };
    let mut pos = 0;
    lengths
        .iter()
        .map(|&len| {
            let item = values[pos..pos + len].to_vec();
            pos += len;
            item
        })
        .collect()
}

fn ids_out() -> SplintrIdsBatch {
    SplintrIdsBatch {
        values: into_ids(Vec::new()),
        lengths: into_lengths(Vec::new()),
    }
}

fn bytes_out() -> SplintrBytesBatch {
    SplintrBytesBatch {
        values: into_bytes(Vec::new()),
        lengths: into_lengths(Vec::new()),
    }
}

fn assert_ids_zero(batch: &SplintrIdsBatch) {
    assert!(batch.values.data.is_null());
    assert_eq!((batch.values.len, batch.values.capacity), (0, 0));
    assert!(batch.lengths.data.is_null());
    assert_eq!((batch.lengths.len, batch.lengths.capacity), (0, 0));
}

fn assert_bytes_zero(batch: &SplintrBytesBatch) {
    assert!(batch.values.data.is_null());
    assert_eq!((batch.values.len, batch.values.capacity), (0, 0));
    assert!(batch.lengths.data.is_null());
    assert_eq!((batch.lengths.len, batch.lengths.capacity), (0, 0));
}

#[test]
fn initializes_uninitialized_outputs() {
    let tokenizer = unsafe { load() };
    let mut out = std::mem::MaybeUninit::<SplintrIdsBatch>::uninit();
    let mut error = std::mem::MaybeUninit::<*mut SplintrError>::uninit();
    let status = unsafe {
        splintr_encode_batch(
            tokenizer,
            b"hello".as_ptr(),
            5,
            [5].as_ptr(),
            1,
            out.as_mut_ptr(),
            error.as_mut_ptr(),
        )
    };
    assert_eq!(status, SPLINTR_OK);
    assert!(unsafe { error.assume_init() }.is_null());
    let out = unsafe { out.assume_init() };
    assert_eq!(unsafe { ids_batch_values(&out) }, vec![vec![2]]);
    unsafe { splintr_ids_batch_free(out) };

    let mut out = std::mem::MaybeUninit::<SplintrBytesBatch>::uninit();
    let mut error = std::mem::MaybeUninit::<*mut SplintrError>::uninit();
    let status = unsafe {
        splintr_decode_batch(
            ptr::null(),
            ptr::null(),
            0,
            ptr::null(),
            0,
            out.as_mut_ptr(),
            error.as_mut_ptr(),
        )
    };
    assert_eq!(status, SPLINTR_ERROR_INVALID_ARGUMENT);
    let out = unsafe { out.assume_init() };
    let error = unsafe { error.assume_init() };
    assert_bytes_zero(&out);
    assert!(!error.is_null());
    unsafe { splintr_bytes_batch_free(out) };
    unsafe { splintr_error_free(error) };
    unsafe { splintr_tokenizer_free(tokenizer) };
}

#[test]
fn batches_match_single_operations_including_empty_items() {
    unsafe {
        let tokenizer = load();
        let texts = ["world", "", "hello tokenizer", "é", "hello", ""];
        let data = texts.concat();
        let lengths: Vec<_> = texts.iter().map(|s| s.len()).collect();
        let mut batch = ids_out();
        assert_eq!(
            splintr_encode_batch(
                tokenizer,
                data.as_ptr(),
                data.len(),
                lengths.as_ptr(),
                lengths.len(),
                &mut batch,
                ptr::null_mut()
            ),
            SPLINTR_OK
        );
        let items = ids_batch_values(&batch);
        assert_eq!(items.len(), texts.len());
        for (text, item) in texts.iter().zip(&items) {
            let mut single = into_ids(Vec::new());
            assert_eq!(
                splintr_encode(tokenizer, text.as_ptr(), text.len(), &mut single, ptr::null_mut()),
                SPLINTR_OK
            );
            let expected = if single.len == 0 {
                &[][..]
            } else {
                slice::from_raw_parts(single.data, single.len)
            };
            assert_eq!(item, expected);
            splintr_ids_free(single);
        }
        let flattened: Vec<u32> = items.iter().flatten().copied().collect();
        let id_lengths: Vec<usize> = items.iter().map(Vec::len).collect();
        let mut decoded = bytes_out();
        assert_eq!(
            splintr_decode_batch(
                tokenizer,
                flattened.as_ptr(),
                flattened.len(),
                id_lengths.as_ptr(),
                id_lengths.len(),
                &mut decoded,
                ptr::null_mut()
            ),
            SPLINTR_OK
        );
        for (item, bytes) in items.iter().zip(bytes_batch_values(&decoded)) {
            let mut single = into_bytes(Vec::new());
            assert_eq!(
                splintr_decode(tokenizer, item.as_ptr(), item.len(), &mut single, ptr::null_mut()),
                SPLINTR_OK
            );
            let expected = if single.len == 0 {
                &[][..]
            } else {
                slice::from_raw_parts(single.data, single.len)
            };
            assert_eq!(bytes, expected);
            splintr_bytes_free(single);
        }
        splintr_ids_batch_free(batch);
        splintr_bytes_batch_free(decoded);

        let mut empty_ids = ids_out();
        let mut empty_bytes = bytes_out();
        assert_eq!(
            splintr_encode_batch(
                tokenizer,
                ptr::null(),
                0,
                ptr::null(),
                0,
                &mut empty_ids,
                ptr::null_mut()
            ),
            SPLINTR_OK
        );
        assert_eq!(
            splintr_decode_batch(
                tokenizer,
                ptr::null(),
                0,
                ptr::null(),
                0,
                &mut empty_bytes,
                ptr::null_mut()
            ),
            SPLINTR_OK
        );
        assert_ids_zero(&empty_ids);
        assert_bytes_zero(&empty_bytes);
        splintr_ids_batch_free(empty_ids);
        splintr_bytes_batch_free(empty_bytes);

        let empty_lengths = [0usize, 0, 0];
        let mut empty_ids = ids_out();
        assert_eq!(
            splintr_encode_batch(
                tokenizer,
                ptr::null(),
                0,
                empty_lengths.as_ptr(),
                empty_lengths.len(),
                &mut empty_ids,
                ptr::null_mut(),
            ),
            SPLINTR_OK
        );
        assert_eq!(ids_batch_values(&empty_ids).len(), 3);
        let mut empty_bytes = bytes_out();
        assert_eq!(
            splintr_decode_batch(
                tokenizer,
                ptr::null(),
                0,
                empty_lengths.as_ptr(),
                empty_lengths.len(),
                &mut empty_bytes,
                ptr::null_mut(),
            ),
            SPLINTR_OK
        );
        assert_eq!(bytes_batch_values(&empty_bytes), vec![b"".to_vec(); 3]);
        splintr_ids_batch_free(empty_ids);
        splintr_bytes_batch_free(empty_bytes);
        splintr_tokenizer_free(tokenizer);
    }
}

#[test]
fn failures_leave_batches_zero_and_owned_errors() {
    unsafe {
        let tokenizer = load();
        let mut ids = ids_out();
        let mut bytes = bytes_out();
        let mut error = ptr::dangling_mut::<SplintrError>();
        let check_ids = |status, ids: &SplintrIdsBatch, error: *mut SplintrError| {
            assert_ne!(status, SPLINTR_OK);
            assert_ids_zero(ids);
            assert!(!error.is_null());
            splintr_error_free(error);
        };
        let check_bytes = |status, bytes: &SplintrBytesBatch, error: *mut SplintrError| {
            assert_ne!(status, SPLINTR_OK);
            assert_bytes_zero(bytes);
            assert!(!error.is_null());
            splintr_error_free(error);
        };
        let split_utf8 = [0xc3, 0xa9];
        let lengths = [1usize, 1];
        let status = splintr_encode_batch(
            tokenizer,
            split_utf8.as_ptr(),
            2,
            lengths.as_ptr(),
            2,
            &mut ids,
            &mut error,
        );
        assert_eq!(status, SPLINTR_ERROR_INVALID_UTF8);
        check_ids(status, &ids, error);
        splintr_ids_batch_free(ids);

        let invalid = [2u32, 999];
        let status = splintr_decode_batch(
            tokenizer,
            invalid.as_ptr(),
            2,
            lengths.as_ptr(),
            2,
            &mut bytes,
            &mut error,
        );
        assert_eq!(status, SPLINTR_ERROR_TOKENIZER);
        check_bytes(status, &bytes, error);
        splintr_bytes_batch_free(bytes);

        let fake = ptr::dangling::<u8>();
        let fake_ids = ptr::dangling::<u32>();
        let fake_lengths = ptr::dangling::<usize>();
        let bad_lengths = [2usize, 2];
        let overflowing = [usize::MAX, 1];
        for (data, len, parts, count) in [
            (fake, 1, ptr::null(), 1),
            (ptr::null(), 1, lengths.as_ptr(), 1),
            (fake, 1, bad_lengths.as_ptr(), 2),
            (fake, 1, overflowing.as_ptr(), 2),
            (fake, 0, lengths.as_ptr(), 1),
            (fake, 1, ptr::null(), 0),
            (
                fake,
                0,
                fake_lengths,
                (isize::MAX as usize) / std::mem::size_of::<usize>() + 1,
            ),
            (fake, isize::MAX as usize + 1, fake_lengths, 0),
        ] {
            ids = ids_out();
            let status = splintr_encode_batch(tokenizer, data, len, parts, count, &mut ids, &mut error);
            assert_eq!(status, SPLINTR_ERROR_INVALID_ARGUMENT);
            check_ids(status, &ids, error);
            splintr_ids_batch_free(ids);
        }
        for (data, len, parts, count) in [
            (fake_ids, 1, ptr::null(), 1),
            (ptr::null(), 1, lengths.as_ptr(), 1),
            (fake_ids, 1, bad_lengths.as_ptr(), 2),
            (fake_ids, 1, overflowing.as_ptr(), 2),
            (fake_ids, 0, lengths.as_ptr(), 1),
            (fake_ids, 1, ptr::null(), 0),
            (
                fake_ids,
                0,
                fake_lengths,
                (isize::MAX as usize) / std::mem::size_of::<usize>() + 1,
            ),
            (
                fake_ids,
                (isize::MAX as usize) / std::mem::size_of::<u32>() + 1,
                fake_lengths,
                0,
            ),
        ] {
            bytes = bytes_out();
            let status = splintr_decode_batch(tokenizer, data, len, parts, count, &mut bytes, &mut error);
            assert_eq!(status, SPLINTR_ERROR_INVALID_ARGUMENT);
            check_bytes(status, &bytes, error);
            splintr_bytes_batch_free(bytes);
        }
        assert_eq!(
            splintr_encode_batch(tokenizer, ptr::null(), 0, ptr::null(), 0, ptr::null_mut(), &mut error),
            SPLINTR_ERROR_INVALID_ARGUMENT
        );
        assert!(!error.is_null());
        splintr_error_free(error);
        splintr_tokenizer_free(tokenizer);
    }
}
