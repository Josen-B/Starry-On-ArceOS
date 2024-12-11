use api::ctypes::*;
use arceos_posix_api as api;

pub(crate) fn sys_clock_gettime(clock_id: i32, tp: *mut timespec) -> i32 {
    unsafe { api::sys_clock_gettime(clock_id, tp) }
}

pub(crate) fn sys_gettimeofday(tv: *mut timeval) -> i32 {
    unsafe { api::sys_gettimeofday(tv) }
}
