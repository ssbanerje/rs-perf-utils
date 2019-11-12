//! Utilties specific to the powerpc64 architecture.

extern "C" {
    #[cfg(target_arch = "powerpc64")]
    fn mfspr_pvr() -> u32;
}

/// Get CPU model string for powerpc64 processors.
#[cfg(target_arch = "powerpc64")]
fn arch_specific_cpustr() -> String {
    let pvr = unsafe { mfspr_pvr() };
    let pvr_version = (pvr >> 16) & 0xFFFF;
    let pvr_revision = (pvr >> 0) & 0xFFFF;
    format!("{:X}{:X}", pvr_version, pvr_revision)
}
