use std::ptr;

fn main() {
    unsafe {
        gtk::gtk_init(ptr::null_mut(), ptr::null_mut());
    }
}
