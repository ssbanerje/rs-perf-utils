//! Utilities to read and write model specific registers (MSRs).

use nix::libc;

#[derive(Debug)]
/// Handle to read and write model specific registers.
///
/// Requires the `msr` kernel module loaded.
pub struct MsrHandle {
    /// File descriptor for MSR device file.
    fd: libc::c_int,
}

impl MsrHandle {
    /// Get a handle to the CPU specific MSR.
    pub fn new(cpuid: u32) -> crate::Result<MsrHandle> {
        match unsafe {
            libc::open(
                format!("/dev/cpu/{}/msr", cpuid).as_ptr() as _,
                libc::O_RDWR,
            )
        } {
            err if err < 0 => Err(crate::Error::from_errno()),
            fd => Ok(MsrHandle { fd }),
        }
    }

    /// Write `value` to `msr`.
    pub fn write(&self, msr: i64, value: u64) {
        unsafe {
            libc::pwrite(
                self.fd,
                &value as *const u64 as _,
                std::mem::size_of_val(&value),
                msr,
            );
        }
    }

    /// Read the value of `msr`.
    pub fn read(&self, msr: i64) -> u64 {
        let mut value = 0u64;
        unsafe {
            libc::pread(
                self.fd,
                &mut value as *const u64 as _,
                std::mem::size_of_val(&value),
                msr,
            );
        }
        value
    }
}

impl Drop for MsrHandle {
    fn drop(&mut self) {
        if self.fd >= 0 {
            unsafe {
                libc::close(self.fd);
            }
        }
    }
}

/// MSR addresses from "Intel 64 and IA-32 Architectures Software Developers Manual Volume 3B: System
/// Programming Guide, Part 2", Appendix A "PERFORMANCE-MONITORING EVENTS"
#[repr(u64)]
#[derive(Debug)]
#[allow(non_camel_case_types, non_snake_case, missing_docs)]
pub enum MsrAddress {
    INST_RETIRED_ANY_ADDR = 0x309,
    CPU_CLK_UNHALTED_THREAD_ADDR = 0x30A,
    CPU_CLK_UNHALTED_REF_ADDR = 0x30B,
    MSR_LDLAT = 0x3F6,
    MSR_FRONTEND = 0x3F7,
    IA32_CR_PERF_GLOBAL_CTRL = 0x38F,
    IA32_CR_FIXED_CTR_CTRL = 0x38D,
    IA32_PERFEVTSEL0_ADDR = 0x186,
    IA32_PERFEVTSEL1_ADDR = 0x186 + 1,
    IA32_PERFEVTSEL2_ADDR = 0x186 + 2,
    IA32_PERFEVTSEL3_ADDR = 0x186 + 3,
    PERF_MAX_FIXED_COUNTERS = 3,
    PERF_MAX_CUSTOM_COUNTERS = 8,
    PERF_MAX_COUNTERS = 3 /* PERF_MAX_FIXED_COUNTERS */ + 8, /* PERF_MAX_CUSTOM_COUNTERS */
    IA32_DEBUGCTL = 0x1D9,
    IA32_PMC0 = 0xC1,
    IA32_PMC1 = 0xC1 + 1,
    IA32_PMC2 = 0xC1 + 2,
    IA32_PMC3 = 0xC1 + 3,
    MSR_OFFCORE_RSP0 = 0x1A6,
    MSR_OFFCORE_RSP1 = 0x1A7,
    PLATFORM_INFO_ADDR = 0xCE,
    IA32_TIME_STAMP_COUNTER = 0x10,
}

impl MsrAddress {
    /// Map MSR addresses to strings
    pub fn msr_map(&self) -> &'static str {
        match self {
            MsrAddress::MSR_LDLAT => "ldlat=",
            MsrAddress::MSR_FRONTEND => "frontend=",
            MsrAddress::MSR_OFFCORE_RSP0 => "offcore_rsp=",
            MsrAddress::MSR_OFFCORE_RSP1 => "offcore_rsp=",
            _ => unimplemented!(),
        }
    }
}
