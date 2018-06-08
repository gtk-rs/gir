pub struct ArgumentList {
    pub(crate) ptr: *mut *mut *mut libc::c_char,
    items: Vec<OsString>,
}

impl ArgumentList {
    pub(crate) fn new(arguments: *mut *mut *mut libc::c_char) -> Self {
        Self {
            ptr: arguments,
            items: unsafe { FromGlibPtrContainer::from_glib_none(ptr::read(arguments)) },
        }
    }

    pub(crate) fn refresh(&mut self) {
        self.items = unsafe { FromGlibPtrContainer::from_glib_none(ptr::read(self.ptr)) };
    }

    // remove the item at index `idx` and shift the raw array
    pub fn remove(&mut self, idx: usize) {
        unsafe {
            let n_args = glib_ffi::g_strv_length(*self.ptr);
            assert!((n_args as usize) == self.items.len());
            assert!((idx as u32) < n_args);

            self.items.remove(idx);

            glib_ffi::g_free(((*self.ptr).offset(idx as isize)) as *mut libc::c_void);

            for i in (idx as u32)..n_args - 1 {
                ptr::write(
                    (*self.ptr).offset(i as isize),
                    *(*self.ptr).offset((i + 1) as isize),
                )
            }
            ptr::write((*self.ptr).offset((n_args - 1) as isize), ptr::null_mut());
        }
    }
}

impl Deref for ArgumentList {
    type Target = [OsString];

    fn deref(&self) -> &Self::Target {
        self.items.as_slice()
    }
}

impl fmt::Debug for ArgumentList {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        self.items.fmt(formatter)
    }
}

impl convert::Into<Vec<OsString>> for ArgumentList {
    fn into(self) -> Vec<OsString> {
        self.items.clone()
    }
}
