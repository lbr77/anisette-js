use std::fmt::Write as _;

use crate::errors::VmError;

pub(crate) fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

pub(crate) fn align_up(value: u64, align: u64) -> u64 {
    if align == 0 {
        return value;
    }
    (value + align - 1) & !(align - 1)
}

pub(crate) fn align_down(value: u64, align: u64) -> u64 {
    if align == 0 {
        return value;
    }
    value & !(align - 1)
}

pub(crate) fn add_i64(base: u64, addend: i64) -> u64 {
    if addend >= 0 {
        base.wrapping_add(addend as u64)
    } else {
        base.wrapping_sub((-addend) as u64)
    }
}

pub(crate) fn as_usize(value: u64) -> Result<usize, VmError> {
    usize::try_from(value).map_err(|_| VmError::IntegerOverflow(value))
}
