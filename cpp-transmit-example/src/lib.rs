use messages::{ApplicationAPI, Message};
use std::ffi::{c_char, c_int, c_uchar, CStr};

#[no_mangle]
pub extern "C" fn generate_transmit_msg(
    buffer: *mut c_uchar,
    path: *const c_char,
    addr: *const c_char,
) -> c_int {
    let path_str = unsafe {
        assert!(!path.is_null());
        CStr::from_ptr(path)
    };

    let addr_str = unsafe {
        assert!(!addr.is_null());
        CStr::from_ptr(addr)
    };

    let msg = Message::ApplicationAPI(ApplicationAPI::TransmitFile {
        path: path_str.to_str().unwrap().to_owned(),
        target_addr: addr_str.to_str().unwrap().to_owned(),
    });
    let msg_bytes = msg.to_bytes();
    unsafe {
        std::slice::from_raw_parts_mut(buffer, msg_bytes.len()).copy_from_slice(&msg_bytes);
    }
    msg_bytes.len().try_into().unwrap()
}
