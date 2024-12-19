use core::ffi::c_char;

use arceos_posix_api as api;
use api::ctypes::stat;

pub(crate) fn sys_getcwd(buf: *mut c_char, size: usize) -> *mut c_char {
    api::sys_getcwd(buf, size)
}

pub(crate) fn sys_openat(dirfd: i32, pathname: *const c_char, flags: i32, mode: u32) -> i32 {
    api::sys_openat(dirfd, pathname, flags, mode)
}

pub(crate) fn sys_close(fd: i32) -> i32 {
    api::sys_close(fd)
}

pub(crate) fn sys_fstat(fd: i32, stat: *mut stat) -> i32 {
    unsafe { api::sys_fstat(fd, stat) }
}
