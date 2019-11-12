use core::arch::x86_64::__cpuid;

/// Get CPU model string for x86_64 processors.
///
/// More information at https://en.wikipedia.org/wiki/CPUID.
#[allow(clippy::cast_ptr_alignment)]
pub fn get_cpu_string() -> String {
    let mut vendor = [0u8; 12];
    let mut family = 0u32;
    let mut model = 0u32;
    let mut step = 0u32;

    let res = unsafe {
        let res = __cpuid(0);
        std::ptr::copy_nonoverlapping(
            &res.ebx as *const u32 as _,
            &mut vendor[0] as *mut u8 as _,
            4,
        );
        std::ptr::copy_nonoverlapping(
            &res.edx as *const u32 as _,
            &mut vendor[4] as *mut u8 as _,
            4,
        );
        std::ptr::copy_nonoverlapping(
            &res.ecx as *const u32 as _,
            &mut vendor[8] as *mut u8 as _,
            4,
        );
        res
    };

    if res.eax >= 1 {
        let res = unsafe { __cpuid(1) };
        step = res.eax & 0xf;
        model = (res.eax >> 4) & 0xf;
        family = (res.eax >> 8) & 0xf;
        if family == 0xf {
            family += (res.eax >> 20) & 0xff;
        }
        if family >= 0x6 {
            model += ((res.eax >> 16) & 0xf) << 4;
        }
    }

    format!(
        "{}-{:X}-{:X}-{:X}",
        std::str::from_utf8(&vendor).unwrap(),
        family,
        model,
        step
    )
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
