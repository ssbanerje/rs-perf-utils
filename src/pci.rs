//! Utilities to read and write to PCIe device files.

use crate::Result;
use std::os::unix::io::AsRawFd;

#[derive(Debug)]
/// Handle to PCI device to read and write data
pub struct PciHandle {
    /// Underlying device file
    file: std::fs::File,
    /// Bus ID
    bus: u32,
    /// Device ID
    device: u32,
    /// Function ID
    function: u32,
}

impl PciHandle {
    /// Create a new PCI handle.
    pub fn new_pci_handle(
        grp_num: Option<u32>,
        bus: u32,
        device: u32,
        function: u32,
    ) -> crate::Result<PciHandle> {
        let bus_str = if let Some(gnum) = grp_num {
            format!("{:4x}:{:2x}", gnum, bus)
        } else {
            format!("{:2x}", bus)
        };
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(format!(
                "/proc/bus/pci/{}/{:2x}.{:2x}",
                bus_str, bus, device,
            ))?;

        Ok(PciHandle {
            file,
            bus,
            device,
            function,
        })
    }

    /// Read `val` from handle at `offset` in the device file.
    ///
    /// This tries to avoid an allocation for val.
    pub fn read(&self, val: &mut [u8], offset: i64) -> Result<usize> {
        nix::sys::uio::pread(self.file.as_raw_fd(), val, offset).map_err(|x| x.into())
    }

    /// Write `val` to handle at `offset` in the device file.
    pub fn write(&self, val: &[u8], offset: i64) -> Result<usize> {
        nix::sys::uio::pwrite(self.file.as_raw_fd(), val, offset).map_err(|x| x.into())
    }
}
