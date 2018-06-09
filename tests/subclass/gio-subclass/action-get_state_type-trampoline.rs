unsafe extern "C" fn action_get_state_type<T: ObjectType>
(ptr: *mut gio_ffi::GAction) -> *const glib_ffi::GVariantType
{
    use std;

    callback_guard!();
    floating_reference_guard!(ptr);
    let klass = &**(ptr as *const *const ClassStruct<T>);
    let interface_static = klass.get_interface_static(gio_ffi::g_action_get_type())
                                     as *const ActionStatic<T>;
    let instance = &*(ptr as *const T::InstanceStructType);
    let imp = instance.get_impl();
    let imp = (*(*interface_static).imp_static).get_impl(imp);
    let wrap = from_glib_borrow(ptr);

    match imp.get_state_type(&wrap){
        Some(t) => t/*Not checked*/.to_glib_none().0,
        None => std::ptr::null()
    }
}
