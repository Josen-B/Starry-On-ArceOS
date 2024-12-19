use api::ctypes::*;
use arceos_posix_api as api;
use api::method::{UtsName, Tms};

pub(crate) fn sys_clock_gettime(clock_id: i32, tp: *mut timespec) -> i32 {
    unsafe { api::sys_clock_gettime(clock_id, tp) }
}

pub(crate) fn sys_gettimeofday(tv: *mut timeval) -> i32 {
    api::sys_gettimeofday(tv)
}

pub(crate) fn sys_uname(ub: *mut UtsName) -> i32 {
    unsafe { api::sys_uname(ub) }
}

pub(crate) fn sys_time(tms: *mut Tms) -> i32 {
    api::sys_time(tms)
}