use alloc::sync::Arc;
use alloc::string::String;
use alloc::collections::btree_map::BTreeMap;
use core::sync::atomic::{Ordering, AtomicU64, AtomicBool};

use axhal::arch::{UspaceContext, read_trapframe_from_kstack, write_trapframe_to_kstack};
use axmm::AddrSpace;
use axsync::Mutex;
use axtask::{current, AxTaskRef, TaskExtRef, TaskId, TaskInner};

use core::sync::atomic::AtomicI32;
use alloc::vec::{self, Vec};
use crate::flags::{SignalNo, AxResult};

/// Task extended data for the monolithic kernel.
pub struct TaskExt {
    /// The process ID.
    pub proc_id: u64,
    /// The clear thread tid field
    ///
    /// See <https://manpages.debian.org/unstable/manpages-dev/set_tid_address.2.en.html#clear_child_tid>
    ///
    /// When the thread exits, the kernel clears the word at this address if it is not NULL.
    clear_child_tid: AtomicU64,
    /// The parent task ID.
    pub parent_id: AtomicU64,
    /// The children of this task.
    pub children: Mutex<Vec<AxTaskRef>>,
    /// The user space context.
    pub uctx: UspaceContext,
    /// The virtual memory address space.
    pub aspace: Arc<Mutex<AddrSpace>>,
}

impl TaskExt {
    pub const fn new(proc_id: u64, uctx: UspaceContext, aspace: Arc<Mutex<AddrSpace>>) -> Self {
        Self {
            proc_id,
            uctx,
            clear_child_tid: AtomicU64::new(0),
            parent_id: AtomicU64::new(1),
            children: Mutex::new(Vec::new()),
            aspace,
        }
    }

    pub(crate) fn pid(&self) -> u64 {
        self.proc_id
    }

    pub(crate) fn clear_child_tid(&self) -> u64 {
        self.clear_child_tid
            .load(Ordering::Relaxed)
    }

    pub(crate) fn set_clear_child_tid(&self, clear_child_tid: u64) {
        self.clear_child_tid
            .store(clear_child_tid, Ordering::Relaxed);
    }

    pub(crate) fn get_parent(&self) -> u64 {
        self.parent_id.load(Ordering::Relaxed)
    }

    pub(crate) fn set_parent(&self, parent_id: u64) {
        self.parent_id
            .store(parent_id, Ordering::Relaxed);
    }

    pub(crate) fn clone_task(
        &self,
        _flags: usize,
        stack: Option<usize>,
        _ptid: usize,
        _tls: usize,
        _ctid: usize,
    ) -> AxResult<u64> {
        let curr = current();
        let mut new_task = TaskInner::new(
            || {
                let curr = current();
                let kstack_top = curr.kernel_stack_top().unwrap();
                info!(
                    "Enter user space: entry={:#x}, ustack={:#x}, kstack={:#x}",
                    curr.task_ext().uctx.get_ip(),
                    curr.task_ext().uctx.get_sp(),
                    kstack_top,
                );
                unsafe { curr.task_ext().uctx.enter_uspace(kstack_top) };
            },
            "sys_clone".into(),
            crate::config::KERNEL_STACK_SIZE,
        );
        //新任务的地址空间和上下文
        let new_aspace = curr.task_ext().aspace.clone();
        //let new_uctx = curr.task_ext().uctx.clone();
        new_task.ctx_mut().set_page_table_root(new_aspace.lock().page_table_root());
        
        let mut trap_frame =
            read_trapframe_from_kstack(curr.get_kernel_stack_top().unwrap());
        trap_frame.set_ret_code(0);
        trap_frame.sepc += 4;

        if let Some(stack) = stack {
            trap_frame.set_user_sp(stack);
        }
        write_trapframe_to_kstack(new_task.get_kernel_stack_top().unwrap(), &trap_frame);
        let new_uctx = UspaceContext::from(&trap_frame);
        let return_id = new_task.id().as_u64();
        new_task.init_task_ext(TaskExt::new(return_id, new_uctx, new_aspace));
        
        let new_task_ref = axtask::spawn_task(new_task);
        curr.task_ext().children.lock().push(new_task_ref);
        Ok(return_id)
    }
}

axtask::def_task_ext!(TaskExt);

pub fn spawn_user_task(aspace: Arc<Mutex<AddrSpace>>, uctx: UspaceContext) -> AxTaskRef {
    let mut task = TaskInner::new(
        || {
            let curr = axtask::current();
            let kstack_top = curr.kernel_stack_top().unwrap();
            info!(
                "Enter user space: entry={:#x}, ustack={:#x}, kstack={:#x}",
                curr.task_ext().uctx.get_ip(),
                curr.task_ext().uctx.get_sp(),
                kstack_top,
            );
            unsafe { curr.task_ext().uctx.enter_uspace(kstack_top) };
        },
        "userboot".into(),
        crate::config::KERNEL_STACK_SIZE,
    );
    task.ctx_mut()
        .set_page_table_root(aspace.lock().page_table_root());
    task.init_task_ext(TaskExt::new(task.id().as_u64(), uctx, aspace));
    axtask::spawn_task(task)
}
