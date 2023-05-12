use messages::{ApplicationAPI, Message};
use std::ffi::{c_char, c_int, c_uchar, CStr};

/// # Safety
///
/// The caller of this function needs to ensure that buffer, path, and addr are not null
/// and that buffer has sufficient space for a message to be written into it.
#[no_mangle]
pub unsafe extern "C" fn generate_transmit_msg(
    buffer: *mut c_uchar,
    cid: *const c_char,
    addr: *const c_char,
) -> c_int {
    let cid_str = unsafe {
        assert!(!cid.is_null());
        CStr::from_ptr(cid)
    };

    let addr_str = unsafe {
        assert!(!addr.is_null());
        CStr::from_ptr(addr)
    };

    let msg = Message::ApplicationAPI(ApplicationAPI::TransmitDag {
        cid: cid_str.to_str().unwrap().to_owned(),
        target_addr: addr_str.to_str().unwrap().to_owned(),
        retries: 0,
    });
    let msg_bytes = msg.to_bytes();
    unsafe {
        std::slice::from_raw_parts_mut(buffer, msg_bytes.len()).copy_from_slice(&msg_bytes);
    }
    msg_bytes.len().try_into().unwrap()
}
