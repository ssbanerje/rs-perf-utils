//! Interfaces that deal with the kernel and userspace perf utilities.

use crate::Error;
use regex::Regex;
use std::path::Path;
use std::process::Command;

/// Wrappers created for the linux kernel userspace headers using `bindgen`.
#[allow(
    clippy::all,
    missing_docs,
    missing_debug_implementations,
    non_upper_case_globals,
    non_camel_case_types,
    non_snake_case
)]
pub mod ffi {
    include!(concat!(env!("OUT_DIR"), "/kernel_headers.rs"));

    // The IOCTLs are defined as macro functions and skipped by bindgen.
    use ioctl_sys::{io, ioc, iow};
    macro_rules! IOCTL {
        ($name:ident, $ty:expr, $nr:expr) => {
            pub const $name: std::os::raw::c_ulong = io!($ty, $nr) as std::os::raw::c_ulong;
        };
        ($name:ident, $ty:expr, $nr:expr, $arg:ty) => {
            pub const $name: std::os::raw::c_ulong =
                iow!($ty, $nr, std::mem::size_of::<$arg>()) as std::os::raw::c_ulong;
        };
    }
    IOCTL!(PERF_EVENT_IOC_ENABLE, b'$', 0);
    IOCTL!(PERF_EVENT_IOC_DISABLE, b'$', 1);
    IOCTL!(PERF_EVENT_IOC_REFRESH, b'$', 2);
    IOCTL!(PERF_EVENT_IOC_RESET, b'$', 3);
    IOCTL!(PERF_EVENT_IOC_PERIOD, b'$', 4, libc::c_ulong);
    IOCTL!(PERF_EVENT_IOC_SET_OUTPUT, b'$', 5);
    IOCTL!(PERF_EVENT_IOC_SET_FILTER, b'$', 6, *mut libc::c_char);
    IOCTL!(PERF_EVENT_IOC_ID, b'$', 7, *mut libc::c_ulong);
    IOCTL!(PERF_EVENT_IOC_SET_BPF, b'$', 8, libc::c_uint);
    IOCTL!(PERF_EVENT_IOC_PAUSE_OUTPUT, b'$', 9, libc::c_uint);
    IOCTL!(
        PERF_EVENT_IOC_QUERY_BPF,
        b'$',
        10,
        *mut perf_event_query_bpf
    );
    IOCTL!(
        PERF_EVENT_IOC_MODIFY_ATTRIBUTES,
        b'$',
        11,
        *mut perf_event_attr
    );

    impl perf_event_attr {
        /// Get the PMU string from the `type_` field of a `perf_event_attr`.
        pub fn get_pmu(&self) -> crate::Result<String> {
            let mut pmus: Vec<crate::Result<String>> = glob::glob("/sys/devices/*/type")
                .unwrap()
                .filter(|entry| match entry {
                    Ok(path) => {
                        let val: u32 = std::fs::read_to_string(path)
                            .unwrap()
                            .trim()
                            .parse()
                            .unwrap();
                        if val == self.type_ {
                            true
                        } else {
                            false
                        }
                    }
                    Err(_) => false,
                })
                .map(|entry| match entry {
                    Ok(path) => {
                        Ok(path.to_str().unwrap().split("/").collect::<Vec<&str>>()[3].into())
                    }
                    Err(_) => panic!(),
                })
                .collect();
            if !pmus.is_empty() {
                pmus.pop().unwrap()
            } else {
                Err(crate::Error::PmuNotFound)
            }
        }

        /// Get the name of an event corresponding to perf's command line tool.
        pub fn get_perf_style_event(&self) -> crate::Result<String> {
            let pmu = self.get_pmu()?;
            let mut perf_evt = String::from(format!("{}/config={:#X}", pmu, self.config));
            unsafe {
                if self.__bindgen_anon_3.config1 != 0 {
                    perf_evt = String::from(format!(
                        "{},config1={:#X}",
                        &perf_evt, self.__bindgen_anon_3.config1
                    ));
                }
                if self.__bindgen_anon_4.config2 as u64 != 0 {
                    perf_evt = String::from(format!(
                        "{},config2={:#X}",
                        &perf_evt, self.__bindgen_anon_4.config2
                    ));
                }
            }
            perf_evt = String::from(format!("{}/", &perf_evt));
            Ok(perf_evt)
        }
    }
}

/// Rust wrapper for the `perf_event_open` system call.
pub fn perf_event_open(
    attr: &ffi::perf_event_attr,
    pid: libc::pid_t,
    cpu: libc::c_int,
    group_fd: libc::c_int,
    flags: libc::c_ulong,
) -> crate::Result<libc::c_int> {
    unsafe {
        match libc::syscall(
            libc::SYS_perf_event_open,
            attr as *const _,
            pid,
            cpu,
            group_fd,
            flags,
        ) {
            -1 => Err(Error::System(-1)),
            rc => Ok(rc as libc::c_int),
        }
    }
}

/// Rust wrapper for the `perf_event_ioc_enable` IOCTL call.
///
/// Used to start counting.
pub fn perf_event_ioc_enable(fd: libc::c_int) -> crate::Result<()> {
    unsafe {
        match libc::ioctl(fd, ffi::PERF_EVENT_IOC_ENABLE) {
            0 => Ok(()),
            rc => Err(Error::System(rc)),
        }
    }
}

/// Rust wrapper for the `perf_event_ioc_disable` IOCTL call.
///
/// Used to stop counting.
pub fn perf_event_ioc_disable(fd: libc::c_int) -> crate::Result<()> {
    unsafe {
        match libc::ioctl(fd, ffi::PERF_EVENT_IOC_DISABLE) {
            0 => Ok(()),
            rc => Err(Error::System(rc)),
        }
    }
}

/// Rust wrapper for the `perf_event_ioc_reset` IOCTL call.
///
/// Used to reset the counter to default.
pub fn perf_event_ioc_reset(fd: libc::c_int) -> crate::Result<()> {
    unsafe {
        match libc::ioctl(fd, ffi::PERF_EVENT_IOC_RESET) {
            0 => Ok(()),
            rc => Err(Error::System(rc)),
        }
    }
}

#[derive(Debug)]
/// Details of the userspace `perf` tool version.
pub struct PerfVersion {
    /// Major version.
    major: i32,
    /// Minor version.
    minor: i32,
}

impl PerfVersion {
    /// Create a new PerfVersion structure directly
    pub fn new(major: i32, minor: i32) -> Self {
        PerfVersion { major, minor }
    }

    /// Create `perf` version structure by parsing the output of the `perf` command.
    pub fn get_details_from_tool() -> crate::Result<Self> {
        let perf_output_buf = Command::new("perf").arg("--version").output()?.stdout;
        let ver_re = Regex::new(r"perf version (\d+)\.(\d+)")?;
        let matches = ver_re
            .captures(std::str::from_utf8(perf_output_buf.as_slice())?)
            .unwrap();
        let major = matches.get(1).unwrap().as_str().parse::<i32>()?;
        let minor = if major > 4 {
            1 << 10 // infinity (hopefully perf versions never reach this high)
        } else {
            matches.get(2).unwrap().as_str().parse::<i32>()?
        };

        Ok(PerfVersion { major, minor })
    }

    /// Get major version.
    #[inline]
    pub fn major(&self) -> i32 {
        self.major
    }

    /// Get minor version
    #[inline]
    pub fn minor(&self) -> i32 {
        self.minor
    }

    /// Allows for direct access.
    #[inline]
    pub fn direct(&self) -> bool {
        self.minor < 4
    }

    /// Can have name attribute in event string.
    #[inline]
    pub fn has_name(&self) -> bool {
        self.minor >= 4
    }

    /// Allows setting offcore response.
    #[inline]
    pub fn offcore(&self) -> bool {
        !self.direct() && Path::new("/sys/devices/cpu/format/offcore_rsp").exists()
    }

    /// Allows setting load latency.
    #[inline]
    pub fn ldlat(&self) -> bool {
        !self.direct() && Path::new("/sys/devices/cpu/format/ldlat").exists()
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
        perf_event_ioc_reset(fd).unwrap();
        perf_event_ioc_enable(fd).unwrap();
        perf_event_ioc_disable(fd).unwrap();
        let mut count: libc::c_ulonglong = 0;
        unsafe {
            let data = &mut count as *mut _ as *mut libc::c_void;
            let size = std::mem::size_of_val(&count);
            libc::read(fd, data, size);
        }
        assert!(count > 0);
    }

    #[test]
    fn test_perf_version() {
        let pv = PerfVersion::get_details_from_tool();
        assert!(pv.is_ok());
    }
}
