//! Interfaces that deal with the kernel and userspace perf utilities.

use crate::{Error, Result};
use nix::libc;

pub mod ffi;
use ffi::*;

mod version;
pub use version::PerfVersion;

/// Rust wrapper for the `perf_event_open` system call.
pub fn perf_event_open(
    attr: &perf_event_attr,
    pid: libc::pid_t,
    cpu: libc::c_int,
    group_fd: libc::c_int,
    flags: libc::c_ulong,
) -> Result<libc::c_int> {
    unsafe {
        match libc::syscall(
            libc::SYS_perf_event_open,
            attr as *const _,
            pid,
            cpu,
            group_fd,
            flags,
        ) {
            -1 => Err(Error::from_errno()),
            rc => Ok(rc as libc::c_int),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::perf::*;

    #[test]
    fn test_perf_events() {
        let paranoid: i8 = std::fs::read_to_string("/proc/sys/kernel/perf_event_paranoid")
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert!(paranoid <= 2);

        let mut attr = ffi::perf_event_attr::default();
        attr.size = std::mem::size_of_val(&attr) as _;
        attr.type_ = ffi::perf_type_id::PERF_TYPE_SOFTWARE as _;
        attr.set_disabled(1);
        attr.set_exclude_kernel(1);
        attr.set_exclude_hv(1);
        attr.config = ffi::perf_sw_ids::PERF_COUNT_SW_TASK_CLOCK as _;
        let fd = perf_event_open(&attr, 0, -1, -1, 0).unwrap();
        unsafe {
            ffi::perf_event_ioc_reset(fd).unwrap();
            ffi::perf_event_ioc_enable(fd).unwrap();
            ffi::perf_event_ioc_disable(fd).unwrap();
        }
        let mut count: libc::c_ulonglong = 0;
        unsafe {
            let data = &mut count as *mut _ as *mut libc::c_void;
            let size = std::mem::size_of_val(&count);
            libc::read(fd, data, size);
        }
        assert!(count > 0);
    }
}
