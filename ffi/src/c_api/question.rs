use crate::c_api::name::CName;
use crate::client::EdnsConfig;
use crate::client::Protocol;
use crate::client::Question;
use std::cell::RefCell;
use std::ffi::c_void;
use std::ffi::CString;
use std::ptr;
use trust_dns_client::rr::Name;
use trust_dns_client::rr::RecordType;

pub type CQuestion = c_void;

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
    max_payload: u16,
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
        max_payload,
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
        Protocol::Udp => "udp",
        Protocol::Tcp => "tcp",
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

#[cfg(test)]
mod tests {
    use super::*;

    use crate::c_api::name::netbase_name_from_ascii;
    use std::ffi::CStr;
    use std::ffi::CString;
    use std::str::FromStr;

    #[test]
    fn rust_lib_works() {
        let question = Question {
            qname: Name::from_str("example.com").unwrap(),
            qtype: RecordType::A,
            proto: Protocol::Udp,
            recursion_desired: false,
            edns_config: None,
        };
        assert_eq!(question.qname, "example.com".parse().unwrap());
        assert_eq!(question.qtype, RecordType::A);
        assert_eq!(question.proto, Protocol::Udp);
    }

    #[test]
    fn c_lib_works() {
        let name_class = CString::new("Netbase::Name").unwrap();
        let question_class = CString::new("Netbase::Question").unwrap();
        let qname = CString::new("example.com").unwrap();
        let name = netbase_name_from_ascii(name_class.as_ptr(), qname.as_ptr() as *mut i8);

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
