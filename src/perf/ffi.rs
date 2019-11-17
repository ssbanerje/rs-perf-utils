//! Wrappers created for the linux kernel userspace headers using `bindgen`.

#![allow(clippy::all,
missing_docs,
missing_debug_implementations,
non_upper_case_globals,
non_camel_case_types,
non_snake_case
)]

use crate::{Error, Result};
use nix::libc;
use nix::{ioctl_none, ioctl_write_int, ioctl_write_ptr};

// Read Bindgen wrappers
include!(concat!(env!("OUT_DIR"), "/kernel_headers.rs"));

// The Ioctls are defined as macro functions and skipped by bindgen.
// Details at https://elixir.bootlin.com/linux/v5.3.10/source/include/uapi/linux/perf_event.h#L456
ioctl_none!(perf_event_ioc_enable, b'$', 0);
ioctl_none!(perf_event_ioc_disable, b'$', 1);
ioctl_none!(perf_event_ioc_refresh, b'$', 2);
ioctl_none!(perf_event_ioc_reset, b'$', 3);
ioctl_write_int!(perf_event_ioc_period, b'$', 4);
ioctl_none!(perf_event_ioc_set_output, b'$', 5);
ioctl_write_ptr!(perf_event_ioc_set_filter, b'$', 6, libc::c_char);
ioctl_write_ptr!(perf_event_ioc_id, b'$', 7, libc::c_ulong);
ioctl_write_int!(perf_event_ioc_set_bpf, b'$', 8);
ioctl_write_int!(perf_event_ioc_pause_output, b'$', 9);
ioctl_write_ptr!(perf_event_ioc_modify_attributes, b'$', 11, perf_event_attr);

/// Rust wrapper for the `perf_event_open` system call.
pub fn perf_event_open(
    attr: &perf_event_attr,
    pid: libc::pid_t,
    cpu: libc::c_int,
    group_fd: libc::c_int,
    flags: libc::c_ulong,
) -> Result<std::os::unix::io::RawFd> {
    unsafe {
        let fd = libc::syscall(
            libc::SYS_perf_event_open,
            attr as *const _,
            pid,
            cpu,
            group_fd,
            flags,
        );
        match fd {
            -1 => Err(Error::from_errno()),
            rc => Ok(rc as _),
        }
    }
}

// Extend perf_event_attr
impl perf_event_attr {
    /// Get the PMU string from the `type_` field of a `perf_event_attr`.
    fn _get_pmu(&self) -> Result<String> {
        let mut pmus: Vec<Result<String>> = glob::glob("/sys/devices/*/type")
            .unwrap()
            .filter_map(|entry| match entry {
                Ok(ref path) => {
                    let val: u32 = std::fs::read_to_string(path)
                        .unwrap()
                        .trim()
                        .parse()
                        .unwrap();
                    if val == self.type_ {
                        let pmu: &str =
                            path.to_str().unwrap().split("/").collect::<Vec<&str>>()[3].into();
                        Some(Ok(pmu.into()))
                    } else {
                        None
                    }
                }
                Err(_) => None,
            })
            .collect();
        if !pmus.is_empty() {
            pmus.pop().unwrap()
        } else {
            Err(Error::PmuNotFound)
        }
    }

    /// Get the event modifiers for this event as a string.
    fn _get_event_modifiers(&self) -> String {
        let mut ret = [b'\0'; 8];
        let mut ctr = 0;
        macro_rules! check_attr {
            ($cond: expr, $val: expr) => {
                if $cond {
                    ret[ctr] = $val;
                    ctr += 1;
                }
            };
        };
        check_attr!(self.exclude_user() == 0, b'u');
        check_attr!(self.exclude_kernel() == 0, b'k');
        check_attr!(self.exclude_hv() == 0, b'h');
        check_attr!(self.exclude_idle() == 0, b'I');
        check_attr!(self.exclude_guest() == 0, b'G');
        check_attr!(self.exclude_host() == 0, b'H');
        check_attr!(self.pinned() != 0, b'D');
        check_attr!(
            self.sample_type == perf_event_sample_format::PERF_SAMPLE_READ as u64,
            b'S'
        );
        let precise = self.precise_ip();
        let prec_string = if precise > 0 && precise < 3 {
            format!("p{}", precise)
        } else if precise == 3 {
            "P".into()
        } else {
            String::default()
        };
        format!(
            "{}{}",
            String::from(std::str::from_utf8(&ret[0..ctr]).unwrap()),
            prec_string
        )
    }

    /// Generate an event string to be used with the `perf` command line tools.
    ///
    /// This string will not match those generated from `PmuEvent`.
    pub fn to_perf_string(&self) -> Result<String> {
        let pmu = self._get_pmu()?;
        let cfg1 = unsafe {
            if self.__bindgen_anon_3.config1 != 0 {
                format!(",config1={:#X}", self.__bindgen_anon_3.config1)
            } else {
                String::default()
            }
        };
        let cfg2 = unsafe {
            if self.__bindgen_anon_4.config2 as u64 != 0 {
                format!(",config2={:#X}", self.__bindgen_anon_4.config2)
            } else {
                String::default()
            }
        };
        Ok(format!(
            "{}/config={:#X}{}{}/{}",
            pmu,
            self.config,
            cfg1,
            cfg2,
            self._get_event_modifiers()
        ))
    }
}

// Extend perf_event_type
impl From<u32> for perf_event_type {
    fn from(val: u32) -> Self {
        if val <= Self::PERF_RECORD_MAX as u32 {
            return unsafe { std::mem::transmute(val) };
        }
        panic!("Trying to parse an invalid perf_event_type");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perf_event_attr_to_str() {
        let mut attr = perf_event_attr::default();
        attr.type_ = perf_type_id::PERF_TYPE_SOFTWARE as _;
        attr.set_disabled(1);
        attr.set_exclude_kernel(1);
        attr.set_exclude_hv(1);
        attr.config = perf_sw_ids::PERF_COUNT_SW_TASK_CLOCK as _;
        let perf_str = attr.to_perf_string();
        assert!(perf_str.is_ok());
        println!("{:?}", perf_str);
    }
}
