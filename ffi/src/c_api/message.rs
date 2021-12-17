use crate::trust_dns_ext::MessageExt;
use std::cell::RefCell;
use std::ffi::c_void;
use std::ffi::CString;
use std::rc::Rc;
use trust_dns_client::op::Message;

pub type CMessage = c_void;

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
