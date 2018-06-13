unsafe extern "C" fn application_open<T: ApplicationBase>(
    ptr: *mut gio_ffi::GApplication,
    files: *mut *mut gio_ffi::GFile,
    num_files: c_int,
    hint: *const c_char,
) where
    T::ImplType: ApplicationImpl<T>,
{
    callback_guard!();
    floating_reference_guard!(ptr);
    let application = &*(ptr as *mut T::InstanceStructType);
    let wrap: T = from_glib_borrow(ptr as *mut T::InstanceStructType);
    let imp = application.get_impl();

    let files_r: Vec<gio::File> = FromGlibContainer::from_glib_none_num(files, num_files as usize);
    let hint_r: String = from_glib_none(hint);
    imp.open(&wrap, &files_r.as_slice(), &hint_r.as_str())
}
