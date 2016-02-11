extern crate gtk_sys;
use std::ptr;

fn main() {
    unsafe {
        gtk_sys::gtk_init(ptr::null_mut(), ptr::null_mut());
    }
}
