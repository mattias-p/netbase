use core::ffi::c_void;
use std::borrow::Borrow;
use std::cell::RefCell;
use std::ffi::CStr;
use std::ffi::CString;
use std::net::IpAddr;
use std::str::FromStr;

pub type CIpAddr = c_void;

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
