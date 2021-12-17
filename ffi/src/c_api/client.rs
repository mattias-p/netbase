use crate::c_api::ip::CIpAddr;
use crate::c_api::message::CMessage;
use crate::c_api::net::CNet;
use crate::c_api::question::CQuestion;
use crate::client::Cache;
use crate::client::Net;
use crate::client::Question;
use std::ffi::c_void;
use std::net::IpAddr;
use std::panic;
use std::ptr;
use std::rc::Rc;

pub type CCache = c_void;

/// Constructs a new cache instance
///
/// # Errors
/// * If a null pointer is returned this means that a panic was caught and the function returned
///   abnormally.
#[no_mangle]
pub extern "C" fn netbase_cache_new() -> *mut CCache {
    let result = panic::catch_unwind(|| Box::into_raw(Box::new(Cache::new())) as *mut CCache);
    match result {
        Ok(this) => this,
        Err(_) => ptr::null_mut(),
    }
}

/// Constructs a new cache instance
///
/// # Arguments
/// * `bytes` - A pointer to the start of serialized data
/// * `size` - Length of the serialized data
/// * `get_buffer` - A callback for getting an error message buffer of required (non-zero) size.
///
/// # Errors
/// * If the callback is called this means an error occurred and that details are found in the
///   buffer.
/// * If the callback is not called and the returned value is a null pointer, this means that a
///   panic was caught and the function returned abnormally.
#[no_mangle]
pub extern "C" fn netbase_cache_from_bytes(
    bytes: *const u8,
    size: usize,
    get_buffer: extern "C" fn(usize) -> *mut u8,
) -> *mut CCache {
    let result = panic::catch_unwind(|| {
        let bytes = ptr::slice_from_raw_parts(bytes, size);
        let bytes = unsafe { &*bytes };
        match Cache::from_vec(bytes.to_vec()) {
            Ok(cache) => Box::into_raw(Box::new(cache)) as *mut CCache,
            Err(err) => {
                let err = err.to_string();
                let buffer = get_buffer(err.len());
                let buffer = ptr::slice_from_raw_parts_mut(buffer, err.len());
                let buffer = unsafe { &mut *buffer };
                buffer.copy_from_slice(err.as_bytes());
                ptr::null_mut()
            }
        }
    });
    match result {
        Ok(this) => this,
        Err(_) => ptr::null_mut(),
    }
}

/// Serializes the cache into a byte string
///
/// # Arguments
/// * `get_buffer` - A callback for getting a buffer of required size.
///   Called exactly once.
///
/// # Errors
/// * If a zero value is returned this means that a panic was caught and the function returned
///   abnormally.
#[no_mangle]
pub extern "C" fn netbase_cache_to_bytes(
    cache: *const CCache,
    get_buffer: extern "C" fn(usize) -> *mut u8,
) -> u8 {
    panic::catch_unwind(|| {
        let cache = unsafe { &*(cache as *const Cache) };
        let bytes = cache.to_vec();
        let size = bytes.len();
        let buffer = get_buffer(size);
        let buffer = ptr::slice_from_raw_parts_mut(buffer, size);
        let buffer = unsafe { &mut *buffer };
        buffer.copy_from_slice(&bytes);
    })
    .is_ok() as u8
}

#[no_mangle]
pub extern "C" fn netbase_cache_lookup(
    cache: *mut CCache,
    net: *const CNet,
    question: *const CQuestion,
    handle_outcome: extern "C" fn(u64, u32, u16, u16, *mut CMessage, *mut CIpAddr),
    server_p: *const *const CIpAddr,
    server_len: usize,
) -> u8 {
    panic::catch_unwind(|| {
        let cache = unsafe { &mut *(cache as *mut Cache) };
        let servers = ptr::slice_from_raw_parts(server_p as *const &IpAddr, server_len);
        let servers = unsafe { &*servers };
        let question = unsafe { &*(question as *const Question) };
        let net = if net.is_null() {
            None
        } else {
            let net = unsafe { &*(net as *const Net) };
            unsafe {
                Rc::increment_strong_count(net);
            }
            let net = unsafe { Rc::from_raw(net) };
            Some(net)
        };

        let mut servers = servers.iter().map(|server| **server).collect();
        let results = cache.lookup(net, question.clone(), &servers);
        for (server, response) in results {
            servers.remove(&server);
            let server = Box::into_raw(Box::new(server)) as *mut CIpAddr;
            match response.outcome {
                Ok((message, size)) => {
                    let kind = 0;
                    let message = Rc::into_raw(message) as *mut CMessage;
                    handle_outcome(
                        response.started,
                        response.duration,
                        size,
                        kind,
                        message,
                        server,
                    );
                }
                Err(kind) => {
                    let size = 0;
                    let kind: u16 = kind.into();
                    let message = ptr::null_mut();
                    handle_outcome(
                        response.started,
                        response.duration,
                        size,
                        kind,
                        message,
                        server,
                    );
                }
            }
        }
        for server in servers {
            let server = Box::into_raw(Box::new(server)) as *mut CIpAddr;
            handle_outcome(0, 0, 0, 0, ptr::null_mut(), server);
        }
    })
    .is_ok() as u8
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn netbase_cache_for_each_request(
    cache: *const CCache,
    callback: extern "C" fn(*mut CIpAddr, *mut CQuestion) -> (),
) -> u8 {
    panic::catch_unwind(|| {
        let cache = unsafe { &*(cache as *const Cache) };
        cache.for_each_request(|(question, server)| {
            let server = Box::into_raw(Box::new(server)) as *mut CIpAddr;
            let question = Box::into_raw(Box::new(question)) as *mut CQuestion;
            callback(server, question);
        });
    })
    .is_ok() as u8
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn netbase_cache_for_each_retry(
    cache: *const CCache,
    question: *const CQuestion,
    server: *const CIpAddr,
    callback: extern "C" fn(u64, u32, u16) -> (),
) -> u8 {
    panic::catch_unwind(|| {
        let cache = unsafe { &*(cache as *const Cache) };
        let server = unsafe { &*(server as *const IpAddr) };
        let question = unsafe { &*(question as *const Question) };
        cache.for_each_retry(question, server, |start, duration, error| {
            callback(start, duration, error.into());
        });
    })
    .is_ok() as u8
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn netbase_cache_DESTROY(p: *mut CCache) -> u8 {
    panic::catch_unwind(|| {
        unsafe { drop(Box::from_raw(p as *mut Cache)) };
    })
    .is_ok() as u8
}
