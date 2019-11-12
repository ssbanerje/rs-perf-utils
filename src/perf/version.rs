use regex::Regex;
use std::path::Path;
use std::process::Command;

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
    use super::*;

    #[test]
    fn test_perf_version() {
        let pv = PerfVersion::get_details_from_tool();
        assert!(pv.is_ok());
    }
}
