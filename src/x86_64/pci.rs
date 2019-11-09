/// Handle to read/write PCI config space using physical memory.
#[derive(Debug)]
pub struct PciHandlePhysicalAddress {
    /// Underlying handle.
    handle: PciHandle,
    /// Base address of the memory region
    base_addr: u64,
}

impl PciHandlePhysicalAddress {
    /// Read data from handle
    #[inline]
    pub fn read<T>(&self, offset: i64, val: &mut T) -> isize {
        self.handle(offset, val)
    }

    /// Write data to handle.
    #[inline]
    pub fn write<T>(&self, offset: i64, val: T) -> isize {
        self.handle(offset, val)
    }
}

/// Header of the PCI Memory Configuration Table.
#[repr(C)]
#[derive(Debug)]
pub struct MCFGHeader {
    signature: [char; 4],
    length: u32,
    revision: char,
    checksum: char,
    oem_id: [char; 6],
    oem_table_id: [char; 8],
    oem_revision: u32,
    creator_id: u32,
    creator_revision: u32,
    reserved: [char; 8]
}

/// Records in the PCI Memory Configuration Table
#[repr(C)]
#[derive(Debug)]
pub struct MCFGRecord {
    base_address: u64,
    pci_segment_group_num: u16,
    start_bus_num: char,
    end_bus_num: char,
    reserved: [char; 4],
}

/// Handle to read/write PCI config space using physical memory using mmaped file I/O.
#[derive(Debug)]
pub struct PciHandleMMAP {
    /// Underlying handle.
    handle: PciHandlePhysicalAddress,
    /// Header of the PCI Memory Configuration Table.
    mcfg_header: MCFGHeader,
    /// Records in the PCI Memory Configuration Table.
    mcfg_records: Vec<MCFGRecord>,
}

impl PciHandleMMAP {
    /// Read data from handle
    #[inline]
    pub fn read<T>(&self, offset: i64, val: &mut T) -> isize {
        self.handle(offset, val)
    }

    /// Write data to handle.
    #[inline]
    pub fn write<T>(&self, offset: i64, val: T) -> isize {
        self.handle(offset, val)
    }
}
