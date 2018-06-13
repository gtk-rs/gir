unsafe extern "C" fn application_local_command_line<T: ApplicationBase>(
    ptr: *mut gio_ffi::GApplication,
    arguments: *mut *mut *mut c_char,
    exit_status: *mut c_int,
) -> glib_ffi::gboolean
where
    T::ImplType: ApplicationImpl<T>,
{
    callback_guard!();
    floating_reference_guard!(ptr);
    let application = &*(ptr as *mut T::InstanceStructType);
    let wrap: T = from_glib_borrow(ptr as *mut T::InstanceStructType);
    let imp = application.get_impl();

    let mut args = ArgumentList::new(arguments);

    match imp.local_command_line(&wrap, &mut args) {
        Some(ret) => {
            ptr::write(exit_status, ret);
            glib_ffi::GTRUE
        }
        None => glib_ffi::GFALSE,
    }
}
