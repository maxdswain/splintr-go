// SPDX-License-Identifier: Apache-2.0

//! Panic-contained C ABI. Ownership and pointer rules: `../include/splintr.h`.
//!
//! Inputs must stay valid for each call. Outputs are exclusive writable storage,
//! disjoint from all inputs, handles, errors and other outputs. Free functions
//! require the original Rust allocation, transferred exactly once.
#![allow(clippy::missing_safety_doc)]
#![deny(unsafe_op_in_unsafe_fn)]

use splintr::{AnyTokenizer, SpecialDecode, SpecialMode, Tokenize};
use std::any::Any;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr;
use std::slice;
use std::str;

pub const SPLINTR_OK: i32 = 0;
pub const SPLINTR_ERROR_INVALID_ARGUMENT: i32 = 1;
pub const SPLINTR_ERROR_INVALID_UTF8: i32 = 2;
pub const SPLINTR_ERROR_TOKENIZER: i32 = 3;
pub const SPLINTR_ERROR_PANIC: i32 = 4;

const SPECIAL_ORDINARY: i32 = 0;
const SPECIAL_ALL: i32 = 1;
const DECODE_SKIP: i32 = 0;
const DECODE_RENDER: i32 = 1;

#[repr(C)]
pub struct SplintrBytes {
    pub data: *mut u8,
    pub len: usize,
    pub capacity: usize,
}

#[repr(C)]
pub struct SplintrIds {
    pub data: *mut u32,
    pub len: usize,
    pub capacity: usize,
}

#[repr(C)]
pub struct SplintrLengths {
    pub data: *mut usize,
    pub len: usize,
    pub capacity: usize,
}

#[repr(C)]
pub struct SplintrIdsBatch {
    pub values: SplintrIds,
    pub lengths: SplintrLengths,
}

#[repr(C)]
pub struct SplintrBytesBatch {
    pub values: SplintrBytes,
    pub lengths: SplintrLengths,
}

/// Opaque outside this crate. The C declaration intentionally exposes no fields.
pub struct SplintrTokenizer(AnyTokenizer);

/// Opaque error whose message is exposed as borrowed length-delimited bytes.
pub struct SplintrError {
    message: String,
}

struct AbiError {
    status: i32,
    message: String,
}

impl AbiError {
    fn invalid(message: impl Into<String>) -> Self {
        Self {
            status: SPLINTR_ERROR_INVALID_ARGUMENT,
            message: message.into(),
        }
    }

    fn utf8(message: impl Into<String>) -> Self {
        Self {
            status: SPLINTR_ERROR_INVALID_UTF8,
            message: message.into(),
        }
    }

    fn tokenizer(error: impl std::fmt::Display) -> Self {
        Self {
            status: SPLINTR_ERROR_TOKENIZER,
            message: error.to_string(),
        }
    }
}

fn panic_message(payload: Box<dyn Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        format!("panic in splintr: {s}")
    } else if let Some(s) = payload.downcast_ref::<String>() {
        format!("panic in splintr: {s}")
    } else {
        "panic in splintr (non-string payload)".to_owned()
    }
}

unsafe fn set_error(out_error: *mut *mut SplintrError, message: String) {
    if !out_error.is_null() {
        // SAFETY: the caller promises that a non-null output points to writable storage.
        unsafe { ptr::write(out_error, Box::into_raw(Box::new(SplintrError { message }))) };
    }
}

/// Validates and initializes a required output before any later failure.
// ABI outputs may be uninitialized and must not alias any other argument.
unsafe fn init_output<'a, T>(output: *mut T, initial: T, null_message: &'static str) -> Result<&'a mut T, AbiError> {
    if output.is_null() {
        return Err(AbiError::invalid(null_message));
    }
    // SAFETY: each ABI function requires its non-null outputs to be writable.
    unsafe { ptr::write(output, initial) };
    // SAFETY: the slot was initialized above and is exclusively owned for this call.
    Ok(unsafe { &mut *output })
}

/// Runs an ABI operation, translating both ordinary errors and Rust panics.
unsafe fn ffi<F>(out_error: *mut *mut SplintrError, operation: F) -> i32
where
    F: FnOnce() -> Result<(), AbiError>,
{
    if !out_error.is_null() {
        // SAFETY: documented output-parameter contract.
        unsafe { ptr::write(out_error, ptr::null_mut()) };
    }
    match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(Ok(())) => SPLINTR_OK,
        Ok(Err(error)) => {
            let status = error.status;
            // SAFETY: same output-parameter contract as above.
            unsafe { set_error(out_error, error.message) };
            status
        }
        Err(payload) => {
            // SAFETY: same output-parameter contract as above.
            unsafe { set_error(out_error, panic_message(payload)) };
            SPLINTR_ERROR_PANIC
        }
    }
}

// Caller guarantees aligned, readable elements that remain unchanged for the borrow.
unsafe fn input_slice<'a, T>(data: *const T, len: usize, name: &str, size_name: &str) -> Result<&'a [T], AbiError> {
    if len > (isize::MAX as usize) / std::mem::size_of::<T>() {
        return Err(AbiError::invalid(format!("{size_name} is too large")));
    }
    if len == 0 {
        return Ok(&[]);
    }
    if data.is_null() {
        return Err(AbiError::invalid(format!("{name} is NULL but its length is non-zero")));
    }
    // SAFETY: checked byte extent; caller guarantees alignment and readable storage.
    Ok(unsafe { slice::from_raw_parts(data, len) })
}

fn nonempty<'a>(bytes: &'a [u8], name: &str) -> Result<&'a [u8], AbiError> {
    if bytes.is_empty() {
        return Err(AbiError::invalid(format!("{name} must not be empty")));
    }
    Ok(bytes)
}

fn utf8<'a>(bytes: &'a [u8], name: &str) -> Result<&'a str, AbiError> {
    str::from_utf8(bytes).map_err(|error| AbiError::utf8(format!("{name} is not UTF-8: {error}")))
}

fn vec_parts<T>(mut value: Vec<T>) -> (*mut T, usize, usize) {
    if value.is_empty() {
        return (ptr::null_mut(), 0, 0);
    }
    let parts = (value.as_mut_ptr(), value.len(), value.capacity());
    std::mem::forget(value);
    parts
}

fn into_bytes(value: Vec<u8>) -> SplintrBytes {
    let (data, len, capacity) = vec_parts(value);
    SplintrBytes { data, len, capacity }
}

fn into_ids(value: Vec<u32>) -> SplintrIds {
    let (data, len, capacity) = vec_parts(value);
    SplintrIds { data, len, capacity }
}

fn into_lengths(value: Vec<usize>) -> SplintrLengths {
    let (data, len, capacity) = vec_parts(value);
    SplintrLengths { data, len, capacity }
}

fn empty_ids_batch() -> SplintrIdsBatch {
    SplintrIdsBatch {
        values: into_ids(Vec::new()),
        lengths: into_lengths(Vec::new()),
    }
}

fn empty_bytes_batch() -> SplintrBytesBatch {
    SplintrBytesBatch {
        values: into_bytes(Vec::new()),
        lengths: into_lengths(Vec::new()),
    }
}

/// Check every partition before borrowing the payload (and before touching any element).
fn validate_partitions(lengths: &[usize], data_len: usize) -> Result<(), AbiError> {
    let mut total = 0usize;
    for &len in lengths {
        total = total
            .checked_add(len)
            .ok_or_else(|| AbiError::invalid("partition lengths overflow"))?;
        if total > data_len {
            return Err(AbiError::invalid("partition lengths exceed data_len"));
        }
    }
    if total != data_len {
        return Err(AbiError::invalid("partition lengths do not sum to data_len"));
    }
    Ok(())
}

fn flatten<T, U>(items: Vec<T>, payload: impl Fn(T) -> U) -> Result<(Vec<U::Item>, Vec<usize>), AbiError>
where
    U: IntoIterator,
    U::IntoIter: ExactSizeIterator,
{
    let mut values = Vec::new();
    let mut lengths = Vec::with_capacity(items.len());
    for item in items {
        let iter = payload(item).into_iter();
        let len = iter.len();
        let total = values
            .len()
            .checked_add(len)
            .ok_or_else(|| AbiError::invalid("batch output overflow"))?;
        if total > (isize::MAX as usize) / std::mem::size_of::<U::Item>() {
            return Err(AbiError::invalid("batch output is too large"));
        }
        values.extend(iter);
        lengths.push(len);
    }
    Ok((values, lengths))
}

unsafe fn tokenizer_ref<'a>(tokenizer: *const SplintrTokenizer) -> Result<&'a AnyTokenizer, AbiError> {
    if tokenizer.is_null() {
        return Err(AbiError::invalid("tokenizer is NULL"));
    }
    // SAFETY: lifetime and concurrent-free requirements are part of the handle contract.
    Ok(&unsafe { &*tokenizer }.0)
}

#[no_mangle]
pub extern "C" fn splintr_go_version_0_1_0() -> u32 {
    0x0001_0000
}

#[no_mangle]
pub unsafe extern "C" fn splintr_tokenizer_from_json(
    json: *const u8,
    json_len: usize,
    out_tokenizer: *mut *mut SplintrTokenizer,
    out_error: *mut *mut SplintrError,
) -> i32 {
    let operation = || {
        // SAFETY: output and input regions obey the ABI contract.
        let output = unsafe { init_output(out_tokenizer, ptr::null_mut(), "out_tokenizer is NULL")? };
        let bytes = unsafe { input_slice(json, json_len, "json", "json length")? };
        let tokenizer = splintr::from_json_bytes(nonempty(bytes, "json")?).map_err(AbiError::tokenizer)?;
        *output = Box::into_raw(Box::new(SplintrTokenizer(tokenizer)));
        Ok(())
    };
    // SAFETY: out_error is a writable ABI output when non-null.
    unsafe { ffi(out_error, operation) }
}

#[no_mangle]
pub unsafe extern "C" fn splintr_tokenizer_from_pretrained(
    name: *const u8,
    name_len: usize,
    out_tokenizer: *mut *mut SplintrTokenizer,
    out_error: *mut *mut SplintrError,
) -> i32 {
    let operation = || {
        // SAFETY: output and input regions obey the ABI contract.
        let output = unsafe { init_output(out_tokenizer, ptr::null_mut(), "out_tokenizer is NULL")? };
        let bytes = unsafe { input_slice(name, name_len, "pretrained name", "pretrained name length")? };
        let name = utf8(nonempty(bytes, "pretrained name")?, "pretrained name")?;
        let tokenizer = splintr::from_pretrained(name).map_err(AbiError::tokenizer)?;
        *output = Box::into_raw(Box::new(SplintrTokenizer(tokenizer)));
        Ok(())
    };
    // SAFETY: out_error is a writable ABI output when non-null.
    unsafe { ffi(out_error, operation) }
}

#[no_mangle]
pub unsafe extern "C" fn splintr_encode(
    tokenizer: *const SplintrTokenizer,
    text: *const u8,
    text_len: usize,
    out_ids: *mut SplintrIds,
    out_error: *mut *mut SplintrError,
) -> i32 {
    unsafe { splintr_encode_with_special_mode(tokenizer, text, text_len, SPECIAL_ALL, out_ids, out_error) }
}

#[no_mangle]
pub unsafe extern "C" fn splintr_encode_with_special_mode(
    tokenizer: *const SplintrTokenizer,
    text: *const u8,
    text_len: usize,
    mode: i32,
    out_ids: *mut SplintrIds,
    out_error: *mut *mut SplintrError,
) -> i32 {
    let operation = || {
        // SAFETY: output, handle and input obey the ABI contract.
        let output = unsafe { init_output(out_ids, into_ids(Vec::new()), "out_ids is NULL")? };
        let tokenizer = unsafe { tokenizer_ref(tokenizer)? };
        let bytes = unsafe { input_slice(text, text_len, "text", "text length")? };
        let text = utf8(bytes, "text")?;
        let mode = match mode {
            SPECIAL_ORDINARY => SpecialMode::Ordinary,
            SPECIAL_ALL => SpecialMode::All,
            _ => return Err(AbiError::invalid("unknown encode special mode")),
        };
        let ids = tokenizer.encode_with(text, &mode).map_err(AbiError::tokenizer)?;
        *output = into_ids(ids);
        Ok(())
    };
    // SAFETY: out_error is a writable ABI output when non-null.
    unsafe { ffi(out_error, operation) }
}

#[no_mangle]
pub unsafe extern "C" fn splintr_decode(
    tokenizer: *const SplintrTokenizer,
    ids: *const u32,
    ids_len: usize,
    out_text: *mut SplintrBytes,
    out_error: *mut *mut SplintrError,
) -> i32 {
    unsafe { splintr_decode_with_special_mode(tokenizer, ids, ids_len, DECODE_SKIP, out_text, out_error) }
}

#[no_mangle]
pub unsafe extern "C" fn splintr_decode_with_special_mode(
    tokenizer: *const SplintrTokenizer,
    ids: *const u32,
    ids_len: usize,
    mode: i32,
    out_text: *mut SplintrBytes,
    out_error: *mut *mut SplintrError,
) -> i32 {
    let operation = || {
        // SAFETY: output, handle and input obey the ABI contract.
        let output = unsafe { init_output(out_text, into_bytes(Vec::new()), "out_text is NULL")? };
        let tokenizer = unsafe { tokenizer_ref(tokenizer)? };
        let ids = unsafe { input_slice(ids, ids_len, "ids", "ids_len")? };
        let mode = match mode {
            DECODE_SKIP => SpecialDecode::Skip,
            DECODE_RENDER => SpecialDecode::Render,
            _ => return Err(AbiError::invalid("unknown decode special mode")),
        };
        let text = tokenizer.decode_with(ids, mode).map_err(AbiError::tokenizer)?;
        *output = into_bytes(text.into_bytes());
        Ok(())
    };
    // SAFETY: out_error is a writable ABI output when non-null.
    unsafe { ffi(out_error, operation) }
}

#[no_mangle]
pub unsafe extern "C" fn splintr_encode_batch(
    tokenizer: *const SplintrTokenizer,
    data: *const u8,
    data_len: usize,
    lengths: *const usize,
    count: usize,
    out: *mut SplintrIdsBatch,
    out_error: *mut *mut SplintrError,
) -> i32 {
    let operation = || {
        // SAFETY: output, handle and input regions obey the ABI contract.
        let output = unsafe { init_output(out, empty_ids_batch(), "out is NULL")? };
        let tokenizer = unsafe { tokenizer_ref(tokenizer)? };
        if data_len > isize::MAX as usize {
            return Err(AbiError::invalid("data_len is too large"));
        }
        let lengths = unsafe { input_slice(lengths, count, "lengths", "count")? };
        validate_partitions(lengths, data_len)?;
        let data = unsafe { input_slice(data, data_len, "data", "data_len")? };
        let mut texts = Vec::with_capacity(count);
        let mut offset = 0;
        for &len in lengths {
            texts.push(utf8(&data[offset..offset + len], "batch item")?);
            offset += len;
        }
        let (values, lengths) = flatten(tokenizer.encode_batch(&texts), |ids| ids)?;
        *output = SplintrIdsBatch {
            values: into_ids(values),
            lengths: into_lengths(lengths),
        };
        Ok(())
    };
    // SAFETY: out_error is a writable ABI output when non-null.
    unsafe { ffi(out_error, operation) }
}

#[no_mangle]
pub unsafe extern "C" fn splintr_decode_batch(
    tokenizer: *const SplintrTokenizer,
    data: *const u32,
    data_len: usize,
    lengths: *const usize,
    count: usize,
    out: *mut SplintrBytesBatch,
    out_error: *mut *mut SplintrError,
) -> i32 {
    let operation = || {
        // SAFETY: output, handle and input regions obey the ABI contract.
        let output = unsafe { init_output(out, empty_bytes_batch(), "out is NULL")? };
        let tokenizer = unsafe { tokenizer_ref(tokenizer)? };
        if data_len > (isize::MAX as usize) / std::mem::size_of::<u32>() {
            return Err(AbiError::invalid("data_len is too large"));
        }
        let lengths = unsafe { input_slice(lengths, count, "lengths", "count")? };
        validate_partitions(lengths, data_len)?;
        let data = unsafe { input_slice(data, data_len, "data", "data_len")? };
        let mut lists = Vec::with_capacity(count);
        let mut offset = 0;
        for &len in lengths {
            lists.push(data[offset..offset + len].to_vec());
            offset += len;
        }
        let decoded = tokenizer.decode_batch(&lists).map_err(AbiError::tokenizer)?;
        let (values, lengths) = flatten(decoded, String::into_bytes)?;
        *output = SplintrBytesBatch {
            values: into_bytes(values),
            lengths: into_lengths(lengths),
        };
        Ok(())
    };
    // SAFETY: out_error is a writable ABI output when non-null.
    unsafe { ffi(out_error, operation) }
}

#[no_mangle]
pub unsafe extern "C" fn splintr_tokenizer_vocab_size(
    tokenizer: *const SplintrTokenizer,
    out_size: *mut usize,
    out_error: *mut *mut SplintrError,
) -> i32 {
    let operation = || {
        // SAFETY: output and handle obey the ABI contract.
        let output = unsafe { init_output(out_size, 0, "out_size is NULL")? };
        *output = unsafe { tokenizer_ref(tokenizer)? }.vocab_size();
        Ok(())
    };
    // SAFETY: out_error is a writable ABI output when non-null.
    unsafe { ffi(out_error, operation) }
}

#[no_mangle]
pub unsafe extern "C" fn splintr_tokenizer_family(
    tokenizer: *const SplintrTokenizer,
    out_family: *mut i32,
    out_error: *mut *mut SplintrError,
) -> i32 {
    let operation = || {
        // SAFETY: output and handle obey the ABI contract.
        let output = unsafe { init_output(out_family, 0, "out_family is NULL")? };
        *output = match unsafe { tokenizer_ref(tokenizer)? }.family() {
            "BPE" => 1,
            "Unigram" => 2,
            "WordPiece" => 3,
            "Spm" => 4,
            _ => 0,
        };
        Ok(())
    };
    // SAFETY: out_error is a writable ABI output when non-null.
    unsafe { ffi(out_error, operation) }
}

#[no_mangle]
pub unsafe extern "C" fn splintr_tokenizer_eos_id(
    tokenizer: *const SplintrTokenizer,
    out_found: *mut u8,
    out_id: *mut u32,
    out_error: *mut *mut SplintrError,
) -> i32 {
    let operation = || {
        if out_found.is_null() || out_id.is_null() {
            return Err(AbiError::invalid("EOS output is NULL"));
        }
        // SAFETY: distinct writable outputs and valid handle are guaranteed by the caller.
        let found = unsafe { init_output(out_found, 0, "EOS output is NULL")? };
        let output = unsafe { init_output(out_id, 0, "EOS output is NULL")? };
        if let Some(id) = unsafe { tokenizer_ref(tokenizer)? }.eos_token_id() {
            *found = 1;
            *output = id;
        }
        Ok(())
    };
    // SAFETY: out_error is a writable ABI output when non-null.
    unsafe { ffi(out_error, operation) }
}

#[no_mangle]
pub unsafe extern "C" fn splintr_tokenizer_special_token_id(
    tokenizer: *const SplintrTokenizer,
    token: *const u8,
    token_len: usize,
    out_found: *mut u8,
    out_id: *mut u32,
    out_error: *mut *mut SplintrError,
) -> i32 {
    let operation = || {
        if out_found.is_null() || out_id.is_null() {
            return Err(AbiError::invalid("special-token output is NULL"));
        }
        // SAFETY: distinct writable outputs, handle and input obey the ABI contract.
        let found = unsafe { init_output(out_found, 0, "special-token output is NULL")? };
        let output = unsafe { init_output(out_id, 0, "special-token output is NULL")? };
        let tokenizer = unsafe { tokenizer_ref(tokenizer)? };
        let bytes = unsafe { input_slice(token, token_len, "special token", "special token length")? };
        let token = utf8(nonempty(bytes, "special token")?, "special token")?;
        if let Some(id) = tokenizer.special_token_id(token) {
            *found = 1;
            *output = id;
        }
        Ok(())
    };
    // SAFETY: out_error is a writable ABI output when non-null.
    unsafe { ffi(out_error, operation) }
}

#[no_mangle]
pub unsafe extern "C" fn splintr_tokenizer_free(tokenizer: *mut SplintrTokenizer) {
    if tokenizer.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: ownership was transferred by a successful constructor.
        drop(unsafe { Box::from_raw(tokenizer) });
    }));
}

// Caller transfers the exact unmodified triple produced by vec_parts, or the NULL empty triple.
unsafe fn reclaim<T>(data: *mut T, len: usize, capacity: usize) {
    if !data.is_null() {
        // SAFETY: caller transfers ownership of the original Vec allocation.
        drop(unsafe { Vec::from_raw_parts(data, len, capacity) });
    }
}

#[no_mangle]
pub unsafe extern "C" fn splintr_bytes_free(buffer: SplintrBytes) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: exact triple returned by into_bytes.
        unsafe { reclaim(buffer.data, buffer.len, buffer.capacity) };
    }));
}

#[no_mangle]
pub unsafe extern "C" fn splintr_ids_free(buffer: SplintrIds) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: exact triple returned by into_ids.
        unsafe { reclaim(buffer.data, buffer.len, buffer.capacity) };
    }));
}

#[no_mangle]
pub unsafe extern "C" fn splintr_ids_batch_free(buffer: SplintrIdsBatch) {
    // Reclaim each field independently so a panic dropping one does not skip the other.
    let _ = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: exact values triple returned by the batch operation.
        unsafe { reclaim(buffer.values.data, buffer.values.len, buffer.values.capacity) };
    }));
    let _ = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: exact lengths triple returned by the batch operation.
        unsafe { reclaim(buffer.lengths.data, buffer.lengths.len, buffer.lengths.capacity) };
    }));
}

#[no_mangle]
pub unsafe extern "C" fn splintr_bytes_batch_free(buffer: SplintrBytesBatch) {
    let _ = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: exact values triple returned by the batch operation.
        unsafe { reclaim(buffer.values.data, buffer.values.len, buffer.values.capacity) };
    }));
    let _ = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: exact lengths triple returned by the batch operation.
        unsafe { reclaim(buffer.lengths.data, buffer.lengths.len, buffer.lengths.capacity) };
    }));
}

#[no_mangle]
pub unsafe extern "C" fn splintr_error_message(
    error: *const SplintrError,
    out_message: *mut *const u8,
    out_len: *mut usize,
) -> i32 {
    match catch_unwind(AssertUnwindSafe(|| {
        if error.is_null() || out_message.is_null() || out_len.is_null() {
            return SPLINTR_ERROR_INVALID_ARGUMENT;
        }
        // SAFETY: pointers satisfy this accessor's documented contract.
        unsafe {
            let message = &(*error).message;
            ptr::write(out_message, message.as_ptr());
            ptr::write(out_len, message.len());
        }
        SPLINTR_OK
    })) {
        Ok(status) => status,
        Err(_) => SPLINTR_ERROR_PANIC,
    }
}

#[no_mangle]
pub unsafe extern "C" fn splintr_error_free(error: *mut SplintrError) {
    if error.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: ownership was transferred through an operation's out_error.
        drop(unsafe { Box::from_raw(error) });
    }));
}

#[cfg(test)]
#[path = "../tests/unit.rs"]
mod tests;
