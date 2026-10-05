// SPDX-License-Identifier: Apache-2.0

//! Panic-contained C ABI. Ownership and pointer rules: `../include/splintr.h`.
#![allow(clippy::missing_safety_doc)]

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
unsafe fn init_output<T>(output: *mut T, initial: T, null_message: &'static str) -> Result<(), AbiError> {
    if output.is_null() {
        return Err(AbiError::invalid(null_message));
    }
    // SAFETY: each ABI function requires its non-null outputs to be writable.
    unsafe { ptr::write(output, initial) };
    Ok(())
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

unsafe fn input_bytes<'a>(data: *const u8, len: usize, name: &str, allow_empty: bool) -> Result<&'a [u8], AbiError> {
    if len > isize::MAX as usize {
        return Err(AbiError::invalid(format!("{name} length is too large")));
    }
    if len == 0 {
        if !allow_empty {
            return Err(AbiError::invalid(format!("{name} must not be empty")));
        }
        return Ok(&[]);
    }
    if data.is_null() {
        return Err(AbiError::invalid(format!("{name} is NULL but its length is non-zero")));
    }
    // SAFETY: the caller promises `len` readable bytes when data is non-null.
    Ok(unsafe { slice::from_raw_parts(data, len) })
}

unsafe fn input_str<'a>(data: *const u8, len: usize, name: &str, allow_empty: bool) -> Result<&'a str, AbiError> {
    // SAFETY: forwards the same caller-owned input region.
    let bytes = unsafe { input_bytes(data, len, name, allow_empty)? };
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
    // SAFETY: `ffi` implements the common output/error contract.
    unsafe {
        ffi(out_error, || {
            init_output(out_tokenizer, ptr::null_mut(), "out_tokenizer is NULL")?;
            let bytes = input_bytes(json, json_len, "json", false)?;
            let tokenizer = splintr::from_json_bytes(bytes).map_err(AbiError::tokenizer)?;
            ptr::write(out_tokenizer, Box::into_raw(Box::new(SplintrTokenizer(tokenizer))));
            Ok(())
        })
    }
}

#[no_mangle]
pub unsafe extern "C" fn splintr_tokenizer_from_pretrained(
    name: *const u8,
    name_len: usize,
    out_tokenizer: *mut *mut SplintrTokenizer,
    out_error: *mut *mut SplintrError,
) -> i32 {
    unsafe {
        ffi(out_error, || {
            init_output(out_tokenizer, ptr::null_mut(), "out_tokenizer is NULL")?;
            let name = input_str(name, name_len, "pretrained name", false)?;
            let tokenizer = splintr::from_pretrained(name).map_err(AbiError::tokenizer)?;
            ptr::write(out_tokenizer, Box::into_raw(Box::new(SplintrTokenizer(tokenizer))));
            Ok(())
        })
    }
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
    unsafe {
        ffi(out_error, || {
            init_output(out_ids, into_ids(Vec::new()), "out_ids is NULL")?;
            let tokenizer = tokenizer_ref(tokenizer)?;
            let text = input_str(text, text_len, "text", true)?;
            let mode = match mode {
                SPECIAL_ORDINARY => SpecialMode::Ordinary,
                SPECIAL_ALL => SpecialMode::All,
                _ => return Err(AbiError::invalid("unknown encode special mode")),
            };
            let ids = tokenizer.encode_with(text, &mode).map_err(AbiError::tokenizer)?;
            ptr::write(out_ids, into_ids(ids));
            Ok(())
        })
    }
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
    unsafe {
        ffi(out_error, || {
            init_output(out_text, into_bytes(Vec::new()), "out_text is NULL")?;
            let tokenizer = tokenizer_ref(tokenizer)?;
            let ids = if ids_len == 0 {
                &[]
            } else {
                if ids_len > (isize::MAX as usize) / std::mem::size_of::<u32>() {
                    return Err(AbiError::invalid("ids_len is too large"));
                }
                if ids.is_null() {
                    return Err(AbiError::invalid("ids is NULL but ids_len is non-zero"));
                }
                slice::from_raw_parts(ids, ids_len)
            };
            let mode = match mode {
                DECODE_SKIP => SpecialDecode::Skip,
                DECODE_RENDER => SpecialDecode::Render,
                _ => return Err(AbiError::invalid("unknown decode special mode")),
            };
            let text = tokenizer.decode_with(ids, mode).map_err(AbiError::tokenizer)?;
            ptr::write(out_text, into_bytes(text.into_bytes()));
            Ok(())
        })
    }
}

#[no_mangle]
pub unsafe extern "C" fn splintr_tokenizer_vocab_size(
    tokenizer: *const SplintrTokenizer,
    out_size: *mut usize,
    out_error: *mut *mut SplintrError,
) -> i32 {
    unsafe {
        ffi(out_error, || {
            init_output(out_size, 0, "out_size is NULL")?;
            ptr::write(out_size, tokenizer_ref(tokenizer)?.vocab_size());
            Ok(())
        })
    }
}

#[no_mangle]
pub unsafe extern "C" fn splintr_tokenizer_family(
    tokenizer: *const SplintrTokenizer,
    out_family: *mut i32,
    out_error: *mut *mut SplintrError,
) -> i32 {
    unsafe {
        ffi(out_error, || {
            init_output(out_family, 0, "out_family is NULL")?;
            let family = match tokenizer_ref(tokenizer)?.family() {
                "BPE" => 1,
                "Unigram" => 2,
                "WordPiece" => 3,
                "Spm" => 4,
                _ => 0,
            };
            ptr::write(out_family, family);
            Ok(())
        })
    }
}

#[no_mangle]
pub unsafe extern "C" fn splintr_tokenizer_eos_id(
    tokenizer: *const SplintrTokenizer,
    out_found: *mut u8,
    out_id: *mut u32,
    out_error: *mut *mut SplintrError,
) -> i32 {
    unsafe {
        ffi(out_error, || {
            if out_found.is_null() || out_id.is_null() {
                return Err(AbiError::invalid("EOS output is NULL"));
            }
            ptr::write(out_found, 0);
            ptr::write(out_id, 0);
            if let Some(id) = tokenizer_ref(tokenizer)?.eos_token_id() {
                ptr::write(out_found, 1);
                ptr::write(out_id, id);
            }
            Ok(())
        })
    }
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
    unsafe {
        ffi(out_error, || {
            if out_found.is_null() || out_id.is_null() {
                return Err(AbiError::invalid("special-token output is NULL"));
            }
            ptr::write(out_found, 0);
            ptr::write(out_id, 0);
            let tokenizer = tokenizer_ref(tokenizer)?;
            let token = input_str(token, token_len, "special token", false)?;
            if let Some(id) = tokenizer.special_token_id(token) {
                ptr::write(out_found, 1);
                ptr::write(out_id, id);
            }
            Ok(())
        })
    }
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

#[no_mangle]
pub unsafe extern "C" fn splintr_bytes_free(buffer: SplintrBytes) {
    if buffer.data.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: exact triple returned by `into_bytes`; caller must not modify it.
        drop(unsafe { Vec::from_raw_parts(buffer.data, buffer.len, buffer.capacity) });
    }));
}

#[no_mangle]
pub unsafe extern "C" fn splintr_ids_free(buffer: SplintrIds) {
    if buffer.data.is_null() {
        return;
    }
    let _ = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: exact triple returned by `into_ids`; caller must not modify it.
        drop(unsafe { Vec::from_raw_parts(buffer.data, buffer.len, buffer.capacity) });
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
