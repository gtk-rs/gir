#![allow(non_camel_case_types)]

use std::ffi::CString;
use std::mem;
use std::ops::Deref;
use std::os::raw::{c_char, c_int, c_uint, c_void};
use std::path::Path;
use std::ptr;
use std::slice;
use std::str;

const GIT_STATUS_OPTIONS_VERSION: c_uint = 1;

type git_repository = *mut c_void;
type git_object = *mut c_void;
type git_status_list = *mut c_void;

#[allow(dead_code)]
#[repr(C)]
enum git_status_opt_t {
    GIT_STATUS_OPT_INCLUDE_UNTRACKED = (1 << 0),
    GIT_STATUS_OPT_INCLUDE_IGNORED = (1 << 1),
    GIT_STATUS_OPT_INCLUDE_UNMODIFIED = (1 << 2),
    GIT_STATUS_OPT_EXCLUDE_SUBMODULES = (1 << 3),
    GIT_STATUS_OPT_RECURSE_UNTRACKED_DIRS = (1 << 4),
    GIT_STATUS_OPT_DISABLE_PATHSPEC_MATCH = (1 << 5),
    GIT_STATUS_OPT_RECURSE_IGNORED_DIRS = (1 << 6),
    GIT_STATUS_OPT_RENAMES_HEAD_TO_INDEX = (1 << 7),
    GIT_STATUS_OPT_RENAMES_INDEX_TO_WORKDIR = (1 << 8),
    GIT_STATUS_OPT_SORT_CASE_SENSITIVELY = (1 << 9),
    GIT_STATUS_OPT_SORT_CASE_INSENSITIVELY = (1 << 10),

    GIT_STATUS_OPT_RENAMES_FROM_REWRITES = (1 << 11),
    GIT_STATUS_OPT_NO_REFRESH = (1 << 12),
    GIT_STATUS_OPT_UPDATE_INDEX = (1 << 13),
    GIT_STATUS_OPT_INCLUDE_UNREADABLE = (1 << 14),
    GIT_STATUS_OPT_INCLUDE_UNREADABLE_AS_UNTRACKED = (1 << 15),
}

#[repr(C)]
struct git_buf {
    ptr: *mut c_char,
    asize: usize,
    size: usize,
}

#[repr(C)]
struct git_status_options {
    version: c_uint,
    show: *mut c_void,
    flags: c_uint,
    pathspec: *mut c_void,
    baseline: *mut c_void,
}

#[link(name = "git2")]
extern "C" {
    fn git_repository_open(out: *mut git_repository, path: *const c_char) -> c_int;
    fn git_revparse_single(
        out: *mut git_object,
        repo: git_repository,
        spec: *const c_char,
    ) -> c_int;
    fn git_object_short_id(out: *mut git_buf, obj: git_object) -> c_int;
    fn git_status_list_new(
        out: *mut git_status_list,
        repo: git_repository,
        opts: *const git_status_options,
    ) -> c_int;
    fn git_status_init_options(opts: *mut git_status_options, version: c_uint) -> c_int;
    fn git_libgit2_init() -> c_int;
    fn git_buf_dispose(buffer: *mut git_buf);
    fn git_repository_free(repo: git_repository);
    fn git_status_list_free(statuslist: git_status_list);
    fn git_status_list_entrycount(statuslist: git_status_list) -> usize;
    fn git_object_free(object: git_object);
}

struct Buf(git_buf);
struct Object(git_object);
struct Repository(git_repository);
struct StatusOptions(git_status_options);

fn init() {
    use std::sync::Once;

    static INIT: Once = Once::new();
    INIT.call_once(|| unsafe {
        if git_libgit2_init() < 0 {
            panic!("Cannot initialize libgit2");
        }
    });
}

impl StatusOptions {
    fn new() -> Option<Self> {
        unsafe {
            let mut options = mem::zeroed();
            if git_status_init_options(&mut options, GIT_STATUS_OPTIONS_VERSION) != 0 {
                return None;
            }
            return Some(Self(options));
        }
    }

    fn flag(&mut self, flag: git_status_opt_t, on: bool) -> &mut StatusOptions {
        if on {
            self.0.flags |= flag as u32;
        } else {
            self.0.flags &= !(flag as u32);
        }
        self
    }

    fn include_untracked(&mut self, include: bool) -> &mut StatusOptions {
        self.flag(git_status_opt_t::GIT_STATUS_OPT_INCLUDE_UNTRACKED, include)
    }

    fn include_ignored(&mut self, include: bool) -> &mut StatusOptions {
        self.flag(git_status_opt_t::GIT_STATUS_OPT_INCLUDE_IGNORED, include)
    }

    fn include_unmodified(&mut self, include: bool) -> &mut StatusOptions {
        self.flag(git_status_opt_t::GIT_STATUS_OPT_INCLUDE_UNMODIFIED, include)
    }
}

impl Buf {
    fn new() -> Self {
        init();
        Buf(git_buf {
            ptr: ptr::null_mut(),
            asize: 0,
            size: 0,
        })
    }

    fn as_str(&self) -> Option<&str> {
        str::from_utf8(&**self).ok()
    }
}

impl Deref for Buf {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        unsafe { slice::from_raw_parts((self.0).ptr as *const u8, (self.0).size as usize) }
    }
}

impl Drop for Buf {
    fn drop(&mut self) {
        unsafe { git_buf_dispose(&mut self.0) }
    }
}

impl Object {
    fn short_id(&self) -> Option<Buf> {
        unsafe {
            let out = Buf::new();
            if git_object_short_id(&out.0 as *const _ as *mut _, self.0) != 0 {
                return None;
            }
            Some(out)
        }
    }
}

impl Drop for Object {
    fn drop(&mut self) {
        unsafe {
            git_object_free(self.0);
        }
    }
}

impl Repository {
    fn open<P: AsRef<Path>>(path: P) -> Option<Self> {
        init();

        let path = CString::new(path.as_ref().to_str()?).ok()?;
        unsafe {
            let mut repository = ptr::null_mut();
            let result = git_repository_open(&mut repository, path.as_ptr());
            if result != 0 {
                return None;
            }
            Some(Self(repository))
        }
    }

    fn revparse_single(&self, spec: &str) -> Option<Object> {
        let mut object = ptr::null_mut();
        let spec = CString::new(spec).ok()?;
        unsafe {
            if git_revparse_single(&mut object, self.0, spec.as_ptr()) != 0 {
                return None;
            }
            Some(Object(object))
        }
    }

    fn status_count(&self, options: &mut StatusOptions) -> Option<usize> {
        unsafe {
            let mut list = ptr::null_mut();
            if git_status_list_new(&mut list, self.0, &options.0) != 0 {
                return None;
            }
            let count = git_status_list_entrycount(list);
            git_status_list_free(list);
            Some(count)
        }
    }
}

impl Drop for Repository {
    fn drop(&mut self) {
        unsafe { git_repository_free(self.0) }
    }
}

pub fn repo_hash<P: AsRef<Path>>(path: P) -> Result<String, ()> {
    if let Some(repo) = Repository::open(path) {
        if let Some(buf) = repo.revparse_single("HEAD").and_then(|obj| obj.short_id()) {
            if let Some(s) = buf.as_str() {
                if dirty(&repo) {
                    return Ok(format!("{}+", s));
                } else {
                    return Ok(s.into());
                }
            }
        }
    }
    Err(())
}

fn dirty(repo: &Repository) -> bool {
    repo.status_count(
        StatusOptions::new()
            .expect("status options")
            .include_ignored(false)
            .include_untracked(false)
            .include_unmodified(false),
    )
    .map(|count| count > 0)
    .unwrap_or(false)
}
