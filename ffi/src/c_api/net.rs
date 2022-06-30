use crate::c_api::ip::CIpAddr;
use crate::c_api::question::CQuestion;
use crate::client::ErrorKind;
use crate::client::Net;
use crate::client::Question;
use std::ffi::c_void;
use std::net::IpAddr;
use std::net::SocketAddr;
use std::ptr;
use std::rc::Rc;
use tokio::runtime::Runtime;

pub type CNet = c_void;

#[no_mangle]
pub extern "C" fn netbase_net_new(
    _class: *const i8,
    bind_addr: *const CIpAddr,
    timeout: u32,
    retry: u16,
    retrans: u32,
) -> *mut CNet {
    let bind_addr = unsafe { *(bind_addr as *const IpAddr) };
    let bind_addr = SocketAddr::new(bind_addr, 0);
    let runtime = Runtime::new().unwrap();
    let net = Rc::new(Net {
        bind_addr,
        timeout,
        retry,
        retrans,
        runtime,
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

    let runtime = Runtime::new().unwrap();
    let _guard = runtime.enter();
    let (_, start, duration, res) = runtime.block_on(net.lookup(question.clone(), server));
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
