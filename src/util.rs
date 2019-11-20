//! Utilities used in this crate.
//!
//! This is not part of the public interface of the crate.

/// Print a hexdump of buffer in memory.
pub fn hexdump(buf: &[u8]) -> String {
    let step = 32;
    let lines: Vec<String> = (0..buf.len())
        .step_by(step)
        .map(|i| {
            let bytes: Vec<String> = (i..std::cmp::min(buf.len(), i + step))
                .map(|x| format!("{:02X}", buf[x]))
                .collect();
            format!("{:?}\t\t{}", &buf[i] as *const u8, bytes.join(" "))
        })
        .collect();
    lines.join("\n")
}

/// Wrappers around rust's volatile reads and writes to memory.
macro_rules! volatile {
    ($id: expr) => {
        unsafe { ::std::ptr::read_volatile(&$id) };
    };
    ($id: expr, $val: expr) => {
        unsafe { ::std::ptr::write_volatile(&mut $id, $val) };
    };
}
