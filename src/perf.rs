#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

/// Wrappers created for the linux kernel userspace headers using `bindgen`.
#[allow(clippy::all)]
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
            -1 => Err(crate::ErrorKind::SystemError.into()),
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
            _ => Err(crate::ErrorKind::SystemError.into()),
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
            _ => Err(crate::ErrorKind::SystemError.into()),
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
            _ => Err(crate::ErrorKind::SystemError.into()),
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
        perf_event_ioc_reset(fd).unwrap();
        perf_event_ioc_enable(fd).unwrap();
        println!("/proc/sys/kernel/perf_event_paranoid -> {}", paranoid);
        perf_event_ioc_disable(fd).unwrap();
        let mut count: libc::c_ulonglong = 0;
        unsafe {
            let data = &mut count as *mut _ as *mut libc::c_void;
            let size = std::mem::size_of_val(&count);
            libc::read(fd, data, size);
        }
        assert!(count > 0);
    }
}

//impl PerfEventAttr {
//    fn _update_config_and_flags(
//        &mut self,
//        cfg: Option<Match>,
//        cfg_qual: Option<Match>,
//        cfg1: Option<Match>,
//        cfg2: Option<Match>,
//        quals: Vec<char>,
//    ) -> Result<()> {
//        self.config = u64::from_str_radix(cfg.unwrap().as_str(), 16)?;
//        if cfg_qual.is_some() {
//            self.config |= u64::from_str_radix(cfg_qual.unwrap().as_str(), 16)?;
//        }
//        if cfg1.is_some() {
//            self.bp_addr_or_config1 |= u64::from_str_radix(cfg1.unwrap().as_str(), 16)?;
//        }
//        if cfg2.is_some() {
//            self.bp_len_or_config2 |= u64::from_str_radix(cfg2.unwrap().as_str(), 16)?;
//        }
//        for q in quals {
//            match q {
//                'p' => {
//                    unimplemented!(); // TODO: Dealing with two bits in the bitfield
//                }
//                'k' => {
//                    self.flags |= PerfAttrFlags::EXCLUDE_USER as u64;
//                }
//                'h' => {
//                    self.flags |= PerfAttrFlags::EXCLUDE_GUEST as u64;
//                }
//                'H' => {
//                    self.flags |= PerfAttrFlags::EXCLUDE_GUEST as u64;
//                }
//                'I' => {
//                    self.flags |= PerfAttrFlags::EXCLUDE_IDLE as u64;
//                }
//                'G' => {
//                    self.flags |= PerfAttrFlags::EXCLUDE_HV as u64;
//                }
//                'D' => {
//                    self.flags |= PerfAttrFlags::PINNED as u64;
//                }
//                _ => {}
//            }
//        }
//        Ok(())
//    }
//
//    /// Create a `perf_event_attr` from the names of perf command line tool events.
//    pub fn from_perf_style_event(perf_evt: String) -> Result<PerfEventAttr> {
//        let mut attr = PerfEventAttr::default();
//        let mut update_flag = false;
//        attr.size = std::mem::size_of::<PerfEventAttr>() as u32;
//        attr.kind = PerfTypeId::PERF_TYPE_RAW as u32;
//
//        // Format 1
//        let pattern = Regex::new(r"r([0-9a-fA-F]+)(:config=([0-9a-fA-F]+)([ukhIGHpPSDW])?(,config1=([0-9a-fA-F]+)?([ukhIGHpPSDW])?(,config2=([0-9a-fA-F]+)([ukhIGHpPSDW])?))?)?")?;
//        let caps = pattern.captures(&perf_evt);
//        if caps.is_some() {
//            let caps = caps.unwrap();
//            let mut qualifiers: Vec<char> = vec![];
//            if caps.get(4).is_some() {
//                qualifiers.push(caps.get(4).unwrap().as_str().chars().next().unwrap())
//            }
//            if caps.get(7).is_some() {
//                qualifiers.push(caps.get(7).unwrap().as_str().chars().next().unwrap())
//            }
//            if caps.get(10).is_some() {
//                qualifiers.push(caps.get(10).unwrap().as_str().chars().next().unwrap())
//            }
//            attr._update_config_and_flags(
//                caps.get(1),
//                caps.get(3),
//                caps.get(6),
//                caps.get(9),
//                qualifiers,
//            )?;
//            update_flag = true;
//        }
//
//        // Format 2
//        let pattern = Regex::new(r"([.^/]+)/([.^/]+)/")?;
//        let caps = pattern.captures(&perf_evt);
//        if caps.is_some() {
//            let caps = caps.unwrap();
//            let pmu = caps.get(1).unwrap().as_str();
//            // TODO: Continue implementation
//
//            update_flag = true;
//        }
//
//        if update_flag {
//            Ok(attr)
//        } else {
//            Err(crate::ErrorKind::BadPerfEvent.into())
//        }
//    }
//
//    /// Get the name of an event corresponding to perf's command line tool.
//    pub fn get_perf_style_event(&self) -> Result<String> {
//        let pmu = self.get_pmu()?;
//        let mut perf_evt = String::from(format!("{}/config={}", pmu, HexVal(self.config)));
//        if self.bp_addr_or_config1 != 0 {
//            perf_evt = String::from(format!(
//                "{},config1={}",
//                &perf_evt,
//                HexVal(self.bp_addr_or_config1)
//            ));
//        }
//        if self.bp_len_or_config2 != 0 {
//            perf_evt = String::from(format!(
//                "{},config2={}",
//                &perf_evt,
//                HexVal(self.bp_len_or_config2)
//            ));
//        }
//        perf_evt = String::from(format!("{}/", &perf_evt));
//        Ok(perf_evt)
//    }
//
//    /// Get the PMU string from the `type`/`kind` field of a `perf_event_attr`.
//    pub fn get_pmu(&self) -> Result<String> {
//        let mut pmus: Vec<Result<String>> = glob("/sys/devices/*/type")?
//            .filter(|entry| match entry {
//                Ok(path) => {
//                    let val: u32 = std::fs::read_to_string(path)
//                        .unwrap()
//                        .trim()
//                        .parse()
//                        .unwrap();
//                    if val == self.kind {
//                        true
//                    } else {
//                        false
//                    }
//                }
//                Err(_) => false,
//            })
//            .map(|entry| match entry {
//                Ok(path) => Ok(path.to_str().unwrap().split("/").collect::<Vec<&str>>()[3].into()),
//                Err(_) => panic!(),
//            })
//            .collect();
//        if !pmus.is_empty() {
//            pmus.pop().unwrap()
//        } else {
//            Err(crate::ErrorKind::PMUNotFound.into())
//        }
//    }
//}
