use arceos_posix_api::{self as api};
use axtask::{current, TaskExtRef};
use num_enum::TryFromPrimitive;
use core::sync::atomic::AtomicI32;
use axstd::thread::yield_now;

use crate::syscall_body;
use crate::flags::{SignalNo, SyscallError, WaitStatus, WaitFlags};
/// ARCH_PRCTL codes
///
/// It is only avaliable on x86_64, and is not convenient
/// to generate automatically via c_to_rust binding.
#[derive(Debug, Eq, PartialEq, TryFromPrimitive)]
#[repr(i32)]
enum ArchPrctlCode {
    /// Set the GS segment base
    SetGs = 0x1001,
    /// Set the FS segment base
    SetFs = 0x1002,
    /// Get the FS segment base
    GetFs = 0x1003,
    /// Get the GS segment base
    GetGs = 0x1004,
    /// The setting of the flag manipulated by ARCH_SET_CPUID
    GetCpuid = 0x1011,
    /// Enable (addr != 0) or disable (addr == 0) the cpuid instruction for the calling thread.
    SetCpuid = 0x1012,
}

pub(crate) fn sys_getpid() -> i32 {
    api::sys_getpid()
}

pub(crate) fn sys_getppid() -> i32 {
    api::sys_getppid()
}

pub(crate) fn sys_exit(status: i32) -> ! {
    let curr = current();
    let clear_child_tid = curr.task_ext().clear_child_tid() as *mut i32;
    if !clear_child_tid.is_null() {
        // TODO: check whether the address is valid
        unsafe {
            // TODO: Encapsulate all operations that access user-mode memory into a unified function
            *(clear_child_tid) = 0;
        }
        // TODO: wake up threads, which are blocked by futex, and waiting for the address pointed by clear_child_tid
    }
    axtask::exit(status);
}

pub(crate) fn sys_exit_group(status: i32) -> ! {
    warn!("Temporarily replace sys_exit_group with sys_exit");
    axtask::exit(status);
}

/// To set the clear_child_tid field in the task extended data.
///
/// The set_tid_address() always succeeds
pub(crate) fn sys_set_tid_address(tid_ptd: *const i32) -> isize {
    syscall_body!(sys_set_tid_address, {
        let curr = current();
        curr.task_ext().set_clear_child_tid(tid_ptd as _);
        Ok(curr.id().as_u64() as isize)
    })
}

#[cfg(target_arch = "x86_64")]
pub(crate) fn sys_arch_prctl(code: i32, addr: u64) -> isize {
    use axerrno::LinuxError;
    syscall_body!(sys_arch_prctl, {
        match ArchPrctlCode::try_from(code) {
            // TODO: check the legality of the address
            Ok(ArchPrctlCode::SetFs) => {
                unsafe {
                    axhal::arch::write_thread_pointer(addr as usize);
                }
                Ok(0)
            }
            Ok(ArchPrctlCode::GetFs) => {
                unsafe {
                    *(addr as *mut u64) = axhal::arch::read_thread_pointer() as u64;
                }
                Ok(0)
            }
            Ok(ArchPrctlCode::SetGs) => {
                unsafe {
                    x86::msr::wrmsr(x86::msr::IA32_KERNEL_GSBASE, addr);
                }
                Ok(0)
            }
            Ok(ArchPrctlCode::GetGs) => {
                unsafe {
                    *(addr as *mut u64) = x86::msr::rdmsr(x86::msr::IA32_KERNEL_GSBASE);
                }
                Ok(0)
            }
            _ => Err(LinuxError::ENOSYS),
        }
    })
}

pub fn sys_clone(flags: usize, stack: usize, ptid: usize, tls: usize, ctid: usize) -> isize {
    syscall_body!(sys_clone, {
        let stack = if stack == 0 {
            None
        } else {
            Some(stack)
        };
        let curr = axtask::current();
        let task_ext = curr.task_ext();
        if let Ok(new_task_id) = task_ext.clone_task(flags, stack, ptid, tls, ctid) {
            Ok(new_task_id as isize)
        } else {
            Err(SyscallError::ENOMEM)
        }
    })
}

pub fn sys_wait4(pid: i32, exit_code_ptr: *mut i32, options: i32) -> isize {
    syscall_body!(sys_wait4, {
        let option = WaitFlags::from_bits(options as u32).unwrap();
        loop {
            let answer = unsafe { wait_pid(pid, exit_code_ptr) };
            match answer {
                Ok(pid) => {
                    return Ok(pid as isize);
                }
                Err(status) => {
                    match status {
                        WaitStatus::NotExist => {
                            return Err(SyscallError::ECHILD);
                        }
                        WaitStatus::Running => {
                            if option.contains(WaitFlags::WNOHANG) {
                                // 不予等待，直接返回0
                                return Ok(0);
                            } else {
                                yield_now();
                            }
                        }
                        _ => {
                            panic!("Shouldn't reach here!");
                        }
                    }
                }
            };
        }
    })
}

pub unsafe fn wait_pid(pid: i32, exit_code_ptr: *mut i32) -> Result<u64, WaitStatus> {
    let curr = axtask::current();
    let curr_task = curr.task_ext();
    let mut exit_task_id: usize = 0;
    let mut answer_id: u64 = 0;
    let mut answer_status = WaitStatus::NotExist;

    for (index, child) in curr_task.children.lock().iter().enumerate() {
        if pid <= 0 {
            if pid == 0 {
                warn!("Don't support for process group.");
            }
            // 任意一个进程结束都可以的
            answer_status = WaitStatus::Running;
            if child.is_exited() {
                let exit_code = child.exit_code();
                answer_status = WaitStatus::Exited;

                exit_task_id = index;
                if !exit_code_ptr.is_null() {
                    unsafe {
                        // 因为没有切换页表，所以可以直接填写
                        *exit_code_ptr = exit_code << 8;
                    }
                }
                answer_id = child.id().as_u64();
                break;
            }
        } else if child.id().as_u64() == pid as u64 {
            // 找到了对应的进程
            if let Some(exit_code) = child.join() {
                answer_status = WaitStatus::Exited;
                exit_task_id = index;
                if !exit_code_ptr.is_null() {
                    unsafe {
                        *exit_code_ptr = exit_code << 8;
                        // 用于WEXITSTATUS设置编码
                    }
                }
                answer_id = child.id().as_u64();
            } else {
                answer_status = WaitStatus::Running;
            }
            break;
        }
    }

    // 若进程成功结束，需要将其从父进程的children中删除
    if answer_status == WaitStatus::Exited {
        curr_task.children.lock().remove(exit_task_id);
        return Ok(answer_id);
    }
    Err(answer_status)
}