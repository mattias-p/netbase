use std::cell::RefCell;
use std::ffi::c_void;
use std::ffi::CStr;
use std::ffi::CString;
use std::ptr;
use trust_dns_client::rr::Name;

pub type CName = c_void;

#[no_mangle]
pub extern "C" fn netbase_name_from_ascii(_class: *const i8, name: *mut i8) -> *mut CName {
    let cstr = unsafe { CStr::from_ptr(name) };
    let bytes = cstr.to_bytes();
    for ch in bytes {
        match ch {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'/' | b'_' | b'.' => continue,
            0x00 => break,
            _ => {
                return ptr::null_mut();
            }
        }
    }
    let bytes: Vec<_> = bytes.to_vec();
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

#[cfg(test)]
mod tests {
    use super::*;

    use std::ffi::CStr;
    use std::ffi::CString;

    #[test]
    fn c_lib_works() {
        let name_class = CString::new("Netbase::Name").unwrap();
        let qname = CString::new("example.com").unwrap();
        let name = netbase_name_from_ascii(name_class.as_ptr(), qname.as_ptr() as *mut i8);
        assert!(!name.is_null());
        let strp = netbase_name_to_string(name);
        assert_eq!(
            unsafe { CStr::from_ptr(strp).to_string_lossy().into_owned() },
            "example.com"
        );
    }
}
