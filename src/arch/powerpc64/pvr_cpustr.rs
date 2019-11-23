//! Utilities to read the PVR register

use log::debug;

extern "C" {
    fn mfspr_pvr() -> u32;
}

/// Get CPU model string for powerpc64 processors.
pub fn get_cpu_string() -> String {
    let pvr = unsafe { mfspr_pvr() };
    let pvr_version = (pvr >> 16) & 0xFFFF;
    let pvr_revision = (pvr >> 0) & 0xFFFF;
    let cpu = format!("{:04x}{:04x}", pvr_version, pvr_revision);
    debug!("Detected powerpc64 processor - {}", cpu);
    cpu
}

#[cfg(test)]
mod tests {
    use super::get_cpu_string;

    #[test]
    fn test_cpu_str() {
        let cpu_str = get_cpu_string();
        assert!(!cpu_str.is_empty());
    }
}
