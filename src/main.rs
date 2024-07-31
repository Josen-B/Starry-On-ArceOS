#![no_std]
#![no_main]

#[macro_use]
extern crate log;
extern crate alloc;
extern crate axstd;

mod loader;
mod mm;
mod syscall;
mod task;
use alloc::sync::Arc;

use axhal::arch::UspaceContext;
use axsync::Mutex;

const USER_STACK_SIZE: usize = 4096;
const KERNEL_STACK_SIZE: usize = 0x40000; // 256 KiB

#[no_mangle]
fn main() {
    let (entry_vaddr, ustack_top, uspace) = mm::load_user_app("hello_world").unwrap();
    let user_task = task::spawn_user_task(
        Arc::new(Mutex::new(uspace)),
        UspaceContext::new(entry_vaddr.into(), ustack_top, 2333),
    );
    let exit_code = user_task.join();
    info!("User task exited with code: {:?}", exit_code);
}
