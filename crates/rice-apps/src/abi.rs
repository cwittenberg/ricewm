use std::sync::{Mutex, OnceLock};

use rice_native_app_protocol::{HostCapabilities, MAX_MESSAGE_BYTES};

use crate::Runtime;

pub const ABI_VERSION: u32 = 1;
pub const MAX_STAGING_BYTES: usize = MAX_MESSAGE_BYTES;
pub const RESULT_ERROR: u32 = u32::MAX;

static RUNTIME: OnceLock<Mutex<Runtime>> = OnceLock::new();
static STAGING: OnceLock<Mutex<Vec<u8>>> = OnceLock::new();
static LAST_ERROR: OnceLock<Mutex<String>> = OnceLock::new();

pub(crate) fn runtime() -> &'static Mutex<Runtime> {
    RUNTIME.get_or_init(|| Mutex::new(Runtime::default()))
}

pub(crate) fn staging() -> &'static Mutex<Vec<u8>> {
    STAGING.get_or_init(|| Mutex::new(Vec::new()))
}

fn last_error() -> &'static Mutex<String> {
    LAST_ERROR.get_or_init(|| Mutex::new(String::new()))
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_apps_abi_version() -> u32 {
    ABI_VERSION
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_apps_init(host_caps_lo: u32, host_caps_hi: u32) -> u32 {
    let capability_bits = u64::from(host_caps_lo) | (u64::from(host_caps_hi) << 32);
    let Ok(mut runtime) = runtime().lock() else {
        return 0;
    };
    *runtime = Runtime::with_capabilities(HostCapabilities::from_bits_retain(capability_bits));
    1
}

/// Rebind host authority after restoring an initialized runtime's memory.
/// Application models and pending protocol state remain intact.
#[unsafe(no_mangle)]
pub extern "C" fn rice_apps_rebind_host_capabilities(host_caps_lo: u32, host_caps_hi: u32) -> u32 {
    let Ok(mut runtime) = runtime().lock() else {
        return 0;
    };
    if !runtime.host_ready || runtime.initialized_capabilities.is_none() {
        return 0;
    }
    let bits = u64::from(host_caps_lo) | (u64::from(host_caps_hi) << 32);
    runtime.initialized_capabilities = Some(HostCapabilities::from_bits_retain(bits));
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_apps_io_reserve(capacity: u32) -> *mut u8 {
    let Ok(capacity) = usize::try_from(capacity) else {
        return std::ptr::null_mut();
    };
    if capacity == 0 || capacity > MAX_STAGING_BYTES {
        return std::ptr::null_mut();
    }
    let Ok(mut staging) = staging().lock() else {
        return std::ptr::null_mut();
    };
    staging.resize(capacity, 0);
    staging.as_mut_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_apps_io_capacity() -> u32 {
    staging()
        .lock()
        .ok()
        .and_then(|staging| u32::try_from(staging.len()).ok())
        .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_apps_handle(input_len: u32) -> u32 {
    let Ok(input_len) = usize::try_from(input_len) else {
        return 0;
    };
    if input_len == 0 || input_len > MAX_STAGING_BYTES {
        return 0;
    }
    let input = {
        let Ok(staging) = staging().lock() else {
            return 0;
        };
        let Some(input) = staging.get(..input_len) else {
            return 0;
        };
        input.to_vec()
    };
    let Ok(mut runtime) = runtime().lock() else {
        return 0;
    };
    match runtime.ingest(&input) {
        Ok(_) => {
            if let Ok(mut message) = last_error().lock() {
                message.clear();
            }
            1
        }
        Err(error) => {
            if let Ok(mut message) = last_error().lock() {
                *message = error.to_string();
            }
            0
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_apps_last_error_len() -> u32 {
    last_error()
        .lock()
        .ok()
        .and_then(|message| u32::try_from(message.len()).ok())
        .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_apps_last_error(output_ptr: *mut u8, capacity: u32) -> u32 {
    let Ok(capacity) = usize::try_from(capacity) else {
        return RESULT_ERROR;
    };
    let Ok(mut staging) = staging().lock() else {
        return RESULT_ERROR;
    };
    if capacity > staging.len()
        || (capacity != 0
            && (output_ptr.is_null() || !std::ptr::eq(output_ptr.cast_const(), staging.as_ptr())))
    {
        return RESULT_ERROR;
    }
    let Ok(message) = last_error().lock() else {
        return RESULT_ERROR;
    };
    if capacity < message.len() {
        return RESULT_ERROR;
    }
    staging[..message.len()].copy_from_slice(message.as_bytes());
    u32::try_from(message.len()).unwrap_or(RESULT_ERROR)
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_apps_pending() -> u32 {
    runtime()
        .lock()
        .ok()
        .and_then(|runtime| u32::try_from(runtime.pending()).ok())
        .unwrap_or(RESULT_ERROR)
}

#[unsafe(no_mangle)]
pub extern "C" fn rice_apps_drain(output_ptr: *mut u8, capacity: u32) -> u32 {
    let Ok(capacity) = usize::try_from(capacity) else {
        return RESULT_ERROR;
    };
    let Ok(mut staging) = staging().lock() else {
        return RESULT_ERROR;
    };
    if capacity > staging.len()
        || (capacity != 0
            && (output_ptr.is_null() || !std::ptr::eq(output_ptr.cast_const(), staging.as_ptr())))
    {
        return RESULT_ERROR;
    }
    let Ok(mut runtime) = runtime().lock() else {
        return RESULT_ERROR;
    };
    runtime
        .drain(&mut staging[..capacity])
        .ok()
        .and_then(|written| u32::try_from(written).ok())
        .unwrap_or(RESULT_ERROR)
}

/// Optional read-only ABI. JSON null means this actor has no qualified document provider.
/// The caller owns the existing staging buffer; querying does not drain app messages.
#[unsafe(no_mangle)]
pub extern "C" fn rice_apps_document_state(
    app_lo: u32,
    app_hi: u32,
    output_ptr: *mut u8,
    capacity: u32,
) -> u32 {
    let Some(app) = rice_native_app_protocol::AppId::new(
        u64::from(app_lo) | (u64::from(app_hi) << 32),
    ) else {
        return RESULT_ERROR;
    };
    let Ok(mut staging) = staging().lock() else {
        return RESULT_ERROR;
    };
    let capacity = capacity as usize;
    if capacity > staging.len()
        || output_ptr.is_null()
        || !std::ptr::eq(output_ptr.cast_const(), staging.as_ptr())
    {
        return RESULT_ERROR;
    }
    let Ok(runtime) = runtime().lock() else {
        return RESULT_ERROR;
    };
    let Ok(bytes) = serde_json::to_vec(&runtime.document_state(app)) else {
        return RESULT_ERROR;
    };
    if bytes.len() > capacity || bytes.len() > 1024 * 1024 {
        return RESULT_ERROR;
    }
    staging[..bytes.len()].copy_from_slice(&bytes);
    bytes.len() as u32
}

/// Bounded language pack ABI; the browser uploads only selected locales.
#[unsafe(no_mangle)]
pub extern "C" fn rice_locale_catalog_loaded(locale: u32) -> u32 {
    u8::try_from(locale).is_ok_and(rice_i18n::browser_catalog::loaded) as u32
}
#[unsafe(no_mangle)]
pub extern "C" fn rice_locale_catalog_begin(locale: u32, bytes: u32) -> *mut u8 {
    rice_i18n::browser_catalog::begin(locale, bytes)
}
#[unsafe(no_mangle)]
pub extern "C" fn rice_locale_catalog_finish(locale: u32, bytes: u32) -> u32 {
    rice_i18n::browser_catalog::finish(locale, bytes) as u32
}
