#[macro_use]
extern crate serde_derive;

mod client;
mod trust_dns_ext;

use crate::client::Cache;
use crate::client::EdnsConfig;
use crate::client::ErrorKind;
use crate::client::Net;
use crate::client::Protocol;
use crate::client::Question;
use crate::trust_dns_ext::MessageExt;
use std::borrow::Borrow;
use std::cell::RefCell;
use std::ffi::c_void;
use std::ffi::CStr;
use std::ffi::CString;
use std::net::IpAddr;
use std::ptr;
use std::rc::Rc;
use std::str::FromStr;
use trust_dns_client::op::Message;
use trust_dns_client::rr::Name;
use trust_dns_client::rr::RecordType;

type CCache = c_void;

#[no_mangle]
pub extern "C" fn netbase_cache_new(_class: *const i8) -> *mut CCache {
    Box::into_raw(Box::new(Cache::new())) as *mut CCache
}

#[no_mangle]
pub extern "C" fn netbase_cache_from_bytes(
    _class: *const i8,
    bytes: *const u8,
    size: usize,
    get_buffer: extern "C" fn(usize) -> *mut u8,
) -> *mut CCache {
    let bytes = ptr::slice_from_raw_parts(bytes, size);
    let bytes = unsafe { &*bytes };
    match Cache::from_vec(bytes.to_vec()) {
        Ok(cache) => Box::into_raw(Box::new(cache)) as *mut CCache,
        Err(err) => {
            let err = err.to_string();
            let buffer = get_buffer(size);
            let buffer = ptr::slice_from_raw_parts_mut(buffer, size);
            let buffer = unsafe { &mut *buffer };
            buffer.copy_from_slice(err.as_bytes());
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn netbase_cache_to_bytes(
    cache: *const CCache,
    get_buffer: extern "C" fn(usize) -> *mut u8,
) {
    let cache = unsafe { &*(cache as *const Cache) };
    let bytes = cache.to_vec();
    let size = bytes.len();
    let buffer = get_buffer(size);
    let buffer = ptr::slice_from_raw_parts_mut(buffer, size);
    let buffer = unsafe { &mut *buffer };
    buffer.copy_from_slice(&bytes);
}

#[no_mangle]
pub extern "C" fn netbase_cache_lookup(
    cache: *mut CCache,
    net: *const CNet,
    question: *const CQuestion,
    handle_outcome: extern "C" fn(u64, u32, u16, u16, *mut CMessage, *mut CIpAddr),
    server_p: *const *const CIpAddr,
    server_len: usize,
) {
    let cache = unsafe { &mut *(cache as *mut Cache) };
    let servers = ptr::slice_from_raw_parts(server_p as *const &IpAddr, server_len);
    let servers = unsafe { &*servers };
    let question = unsafe { &*(question as *const Question) };
    let net = if net == std::ptr::null() {
        None
    } else {
        let net = unsafe { &*(net as *const Net) };
        unsafe {
            Rc::increment_strong_count(net);
        }
        let net = unsafe { Rc::from_raw(net) };
        Some(net)
    };

    let server = servers[0];
    let server_out = Box::into_raw(Box::new(*server)) as *mut CIpAddr;
    if let Some((start, duration, res)) = cache.lookup(net, question.clone(), *server) {
        match res {
            Ok((message, size)) => {
                let kind = 0;
                let message = Rc::into_raw(message) as *mut CMessage;
                handle_outcome(start, duration, size, kind, message, server_out);
            }
            Err(kind) => {
                let size = 0;
                let kind: u16 = kind.into();
                let message = std::ptr::null_mut();
                handle_outcome(start, duration, size, kind, message, server_out);
            }
        }
    }
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn netbase_cache_for_each_request(
    cache: *const CCache,
    callback: extern "C" fn(*mut CIpAddr, *mut CQuestion) -> (),
) {
    let cache = unsafe { &*(cache as *const Cache) };
    cache.for_each_request(|(question, server)| {
        let server = Box::into_raw(Box::new(server)) as *mut CIpAddr;
        let question = Box::into_raw(Box::new(question)) as *mut CQuestion;
        callback(server, question);
    });
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn netbase_cache_for_each_retry(
    cache: *const CCache,
    question: *const CQuestion,
    server: *const CIpAddr,
    callback: extern "C" fn(u64, u32, u16) -> (),
) {
    let cache = unsafe { &*(cache as *const Cache) };
    let server = unsafe { &*(server as *const IpAddr) };
    let question = unsafe { &*(question as *const Question) };
    cache.for_each_retry(question, server, |start, duration, error| {
        callback(start, duration, error.into());
    });
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn netbase_cache_DESTROY(p: *mut CCache) {
    unsafe { drop(Box::from_raw(p as *mut Cache)) };
}

type CNet = c_void;

#[no_mangle]
pub extern "C" fn netbase_net_new(
    _class: *const i8,
    timeout: u32,
    retry: u16,
    retrans: u32,
) -> *mut CNet {
    let net = Rc::new(Net {
        timeout,
        retry,
        retrans,
    });
    Rc::into_raw(net) as *mut CNet
}

#[no_mangle]
pub extern "C" fn netbase_net_lookup(
    net: *mut CNet,
    question: *const CQuestion,
    server: *const CIpAddr,
    query_start: *mut u64,
    query_duration: *mut u32,
    get_buffer: extern "C" fn(usize) -> *mut u8,
) -> u16 {
    let net = unsafe { &mut *(net as *mut Net) };
    let server = unsafe { *(server as *const IpAddr) };
    let question = unsafe { &*(question as *const Question) };
    let (_, start, duration, res) = net.lookup(question.clone(), server);
    unsafe {
        *query_start = start;
    };
    unsafe {
        *query_duration = duration;
    }
    match res {
        Ok(bytes) => {
            let buf = get_buffer(bytes.len());
            let buf = ptr::slice_from_raw_parts_mut(buf, bytes.len());
            let buf = unsafe { &mut *buf };
            buf.copy_from_slice(&bytes);
            0
        }
        Err(error) => ErrorKind::from(&error).into(),
    }
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn netbase_net_DESTROY(net: *mut CNet) {
    let net = unsafe { Rc::from_raw(net as *mut Net) };
    drop(net);
}

type CIpAddr = c_void;

#[no_mangle]
pub extern "C" fn netbase_ip_new(_class: *const i8, ip: *const i8) -> *mut CIpAddr {
    let ip = unsafe { CStr::from_ptr(ip) };
    let ip = ip.to_string_lossy();
    let ip: &str = ip.borrow();
    if let Ok(ip) = IpAddr::from_str(ip) {
        return Box::into_raw(Box::new(ip)) as *mut CIpAddr;
    }
    std::ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn netbase_ip_to_string(ip: *mut CIpAddr) -> *const i8 {
    thread_local!(
        static KEEP: RefCell<Option<CString>> = RefCell::new(None);
    );

    let ip = unsafe { &*(ip as *mut IpAddr) };
    let output = ip.to_string();
    let output = CString::new(output).unwrap();
    let ptr = output.as_ptr();
    KEEP.with(|k| {
        *k.borrow_mut() = Some(output);
    });
    ptr
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn netbase_ip_DESTROY(p: *mut CIpAddr) {
    unsafe { drop(Box::from_raw(p as *mut IpAddr)) };
}

type CName = c_void;

#[no_mangle]
pub extern "C" fn netbase_name_from_ascii(_class: *const i8, name: *mut i8) -> *mut CName {
    let cstr = unsafe { CStr::from_ptr(name) };
    let bytes = cstr.to_bytes();
    for ch in bytes {
        match ch {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'/' | b'_' | b'.' => continue,
            0x00 => break,
            _ => {
                return std::ptr::null_mut();
            }
        }
    }
    let bytes: Vec<_> = bytes.iter().cloned().collect();
    let name = unsafe { String::from_utf8_unchecked(bytes) };
    let name = Name::from_ascii(&name).unwrap();
    Box::into_raw(Box::new(name)) as *mut CName
}

#[no_mangle]
pub extern "C" fn netbase_name_to_string(this: *mut CName) -> *const i8 {
    thread_local!(
        static KEEP: RefCell<Option<CString>> = RefCell::new(None);
    );

    let this = unsafe { &*(this as *mut Name) };
    let output = CString::new(this.to_string()).unwrap();
    let ptr = output.as_ptr();
    KEEP.with(|k| {
        *k.borrow_mut() = Some(output);
    });
    ptr
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn netbase_name_DESTROY(this: *mut CName) {
    unsafe { drop(Box::from_raw(this as *mut Name)) };
}

type CQuestion = c_void;

#[no_mangle]
pub extern "C" fn netbase_question_new(
    _class: *const i8,
    qname: *const CName,
    qtype: u16,
    proto: u8,
    recursion_desired: u8,
) -> *mut CName {
    let qname = unsafe { &*(qname as *const Name) };
    let qtype = RecordType::from(qtype);
    let recursion_desired = recursion_desired != 0;
    if let Ok(proto) = Protocol::try_from(proto) {
        let question = Question {
            qname: qname.clone(),
            qtype,
            proto,
            recursion_desired,
            edns_config: None,
        };
        Box::into_raw(Box::new(question)) as *mut CName
    } else {
        std::ptr::null_mut()
    }
}

#[no_mangle]
pub extern "C" fn netbase_question_set_edns(
    this: *mut CQuestion,
    version: u8,
    dnssec_ok: u8,
    option_code: u16,
    option_value: *const u8,
    option_value_len: usize,
) {
    eprintln!("len {:08x?}", option_value_len);
    let this = unsafe { &mut *(this as *mut Question) };
    let dnssec_ok = dnssec_ok != 0;
    let option_value = ptr::slice_from_raw_parts(option_value, option_value_len);
    let option_value_slice = unsafe { &*option_value };
    let option_value = {
        let mut tmp = Vec::with_capacity(option_value_len);
        tmp.extend_from_slice(option_value_slice);
        tmp
    };

    this.edns_config = Some(EdnsConfig {
        version,
        dnssec_ok,
        option_code,
        option_value,
    });
}

#[no_mangle]
pub extern "C" fn netbase_question_to_string(this: *mut CQuestion) -> *const i8 {
    thread_local!(
        static KEEP: RefCell<Option<CString>> = RefCell::new(None);
    );

    let this = unsafe { &*(this as *mut Question) };
    let recurse = if this.recursion_desired { "" } else { "no" };
    let proto = match this.proto {
        Protocol::UDP => "udp",
        Protocol::TCP => "tcp",
    };

    let output = if let Some(edns_config) = &this.edns_config {
        let dnssec = if edns_config.dnssec_ok { "" } else { "no" };
        let (ednsopt, ednsopt_code) = if edns_config.option_code != 0 {
            ("", format!(" {}", edns_config.option_code))
        } else {
            ("no", "".to_string())
        };
        format!(
            "{} {} +{}recurse +edns {} +{}dnssec +{}ednsopt{} +{}",
            &this.qname,
            this.qtype,
            recurse,
            edns_config.version,
            dnssec,
            ednsopt,
            ednsopt_code,
            proto,
        )
    } else {
        format!(
            "{} {} +{}recurse +noedns +{}",
            &this.qname, this.qtype, recurse, proto,
        )
    };

    let output = CString::new(output).unwrap();
    let ptr = output.as_ptr();
    KEEP.with(|k| {
        *k.borrow_mut() = Some(output);
    });
    ptr
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn netbase_question_DESTROY(p: *mut CQuestion) {
    unsafe { drop(Box::from_raw(p as *mut Question)) };
}

type CMessage = c_void;

#[no_mangle]
pub extern "C" fn netbase_message_new(_class: *const i8) -> *mut CMessage {
    Rc::into_raw(Rc::new(Message::new())) as *mut CMessage
}

#[no_mangle]
pub extern "C" fn netbase_message_to_string(this: *mut CMessage) -> *const i8 {
    thread_local!(
        static KEEP: RefCell<Option<CString>> = RefCell::new(None);
    );

    let this = unsafe { &*(this as *mut Message) };
    let output = format!("{}", this.as_dig());
    let output = CString::new(output).unwrap();
    let ptr = output.as_ptr();
    KEEP.with(|k| {
        *k.borrow_mut() = Some(output);
    });
    ptr
}

#[allow(non_snake_case)]
#[no_mangle]
pub extern "C" fn netbase_message_DESTROY(p: *mut CMessage) {
    unsafe { drop(Rc::from_raw(p as *mut Message)) };
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::ffi::CStr;
    use std::ffi::CString;

    #[test]
    fn rust_lib_works() {
        let question = Question {
            qname: Name::from_str("example.com").unwrap(),
            qtype: RecordType::A,
            proto: Protocol::UDP,
            recursion_desired: false,
            edns_config: None,
        };
        assert_eq!(question.qname, "example.com".parse().unwrap());
        assert_eq!(question.qtype, RecordType::A);
        assert_eq!(question.proto, Protocol::UDP);
    }

    #[test]
    fn c_lib_works() {
        let name_class = CString::new("Netbase::Name").unwrap();
        let question_class = CString::new("Netbase::Question").unwrap();

        let qname = CString::new("example.com").unwrap();
        let name = netbase_name_from_ascii(name_class.as_ptr(), qname.as_ptr() as *mut i8);
        assert!(name != std::ptr::null_mut());
        let strp = netbase_name_to_string(name);
        assert_eq!(
            unsafe { CStr::from_ptr(strp).to_string_lossy().into_owned() },
            "example.com"
        );
        let rrtype_a = 1;
        let question = netbase_question_new(question_class.as_ptr(), name, rrtype_a, 1, 1);
        assert_eq!(
            unsafe {
                CStr::from_ptr(netbase_question_to_string(question))
                    .to_string_lossy()
                    .into_owned()
            },
            "example.com A +recurse +noedns +udp"
        );
    }
}
