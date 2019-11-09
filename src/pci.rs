#[derive(Debug)]
/// Handle to PCI device to read and write data
pub struct PciHandle {
    /// File descriptor to underlying device file
    fd: libc::c_int,
    /// Bus ID
    bus: u32,
    /// Device ID
    device: u32,
    /// Function ID
    function: u32,
}

impl PciHandle {
    /// Create a new PCI handle
    pub fn new_pci_handle(
        grp_num: Option<u32>,
        bus: u32,
        device: u32,
        function: u32,
    ) -> crate::Result<PciHandle> {
        let path = if let Some(gnum) = grp_num {
            format!(
                "/proc/bus/pci/{:4X}:{:2X}/{:2X}.{:2X}",
                gnum, bus, device, function
            )
        } else {
            format!("/proc/bus/pci/{:2X}/{:2X}.{:2X}", bus, device, function)
        };

        match unsafe { libc::open(path.as_ptr() as _, libc::O_RDWR) } {
            err if err < 0 => Err(crate::Error::SystemError(err)),
            fd => Ok(PciHandle {
                fd,
                bus,
                device,
                function,
            }),
        }
    }

    /// Read data from handle
    pub fn read<T>(&self, offset: i64, val: &mut T) -> isize {
        unsafe {
            libc::pread(
                self.fd,
                val as *mut T as _,
                std::mem::size_of::<T>(),
                offset,
            )
        }
    }

    /// Write data to handle.
    pub fn write<T>(&self, offset: i64, val: T) -> isize {
        unsafe {
            libc::pwrite(
                self.fd,
                &val as *const T as _,
                std::mem::size_of::<T>(),
                offset,
            )
        }
    }
}

impl Drop for PciHandle {
    fn drop(&mut self) {
        if self.fd > 0 {
            unsafe {
                libc::close(self.fd);
            }
        }
    }
}
