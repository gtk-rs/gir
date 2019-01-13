unsafe extern "C" fn action_get_state<T: ObjectType>
(gptr: *mut gio_ffi::GAction) -> *mut glib_ffi::GVariant
{
    callback_guard!();
    floating_reference_guard!(gptr);
    let klass = &**(gptr as *const *const ClassStruct<T>);
    let interface_static = klass.get_interface_static(gio_ffi::g_action_get_type())
                                     as *const ActionStatic<T>;
    let instance = &*(gptr as *const T::InstanceStructType);
    let imp = instance.get_impl();
    let imp = (*(*interface_static).imp_static).get_impl(imp);
    let wrap = from_glib_borrow(gptr);

    let ret = imp.get_state(&wrap);
    let gptr = ret.to_glib_none().0;
    mem::forget(ret);
    gptr
}
