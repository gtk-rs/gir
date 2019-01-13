unsafe extern "C" fn action_activate<T: ObjectType>
(gptr: *mut gio_ffi::GAction, parameter: *mut glib_ffi::GVariant)
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

    let param = if parameter.is_null(){
        None
    }else{
        Some(from_glib_none(parameter))
    };
    imp.activate(&wrap, param.as_ref())
}
