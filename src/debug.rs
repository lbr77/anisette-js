use std::fmt::Write as _;

use unicorn_engine::unicorn_const::MemType;
use unicorn_engine::{RegisterARM64, Unicorn};

use crate::constants::{DEBUG_PRINT_ENABLED, DEBUG_TRACE_ENABLED};
use crate::runtime::RuntimeState;


pub(crate) fn debug_print(message: impl AsRef<str>) {
    if DEBUG_PRINT_ENABLED {
        println!("{}", message.as_ref());
    }
}

pub(crate) fn debug_trace(message: impl AsRef<str>) {
    if DEBUG_TRACE_ENABLED {
        println!("{}", message.as_ref());
    }
}

pub(crate) fn reg_or_zero(uc: &Unicorn<'_, RuntimeState>, reg: RegisterARM64) -> u64 {
    uc.reg_read(reg).unwrap_or(0)
}

pub(crate) fn trace_mem_invalid_hook(
    uc: &Unicorn<'_, RuntimeState>,
    access: MemType,
    address: u64,
    size: usize,
    value: i64,
) {
    let pc = reg_or_zero(uc, RegisterARM64::PC);
    match access {
        MemType::READ_UNMAPPED => {
            println!(
                ">>> Missing memory is being READ at 0x{address:x}, data size = {size}, data value = 0x{:x}, PC=0x{pc:x}",
                value as u64
            );
            dump_registers(uc, "read unmapped");
        }
        MemType::WRITE_UNMAPPED => {
            println!(
                ">>> Missing memory is being WRITE at 0x{address:x}, data size = {size}, data value = 0x{:x}, PC=0x{pc:x}",
                value as u64
            );

            dump_registers(uc, "write unmapped");
        }
        MemType::FETCH_UNMAPPED => {
            println!(
                ">>> Missing memory is being FETCH at 0x{address:x}, data size = {size}, data value = 0x{:x}, PC=0x{pc:x}",
                value as u64
            );
        }
        _ => {}
    }
}

pub(crate) fn dump_registers(uc: &Unicorn<'_, RuntimeState>, label: &str) {
    // debug_print(format!());
    println!("REGDUMP {label}");

    let regs: &[(RegisterARM64, &str)] = &[
        (RegisterARM64::X0, "X0"),
        (RegisterARM64::X1, "X1"),
        (RegisterARM64::X2, "X2"),
        (RegisterARM64::X3, "X3"),
        (RegisterARM64::X4, "X4"),
        (RegisterARM64::X5, "X5"),
        (RegisterARM64::X6, "X6"),
        (RegisterARM64::X7, "X7"),
        (RegisterARM64::X8, "X8"),
        (RegisterARM64::X9, "X9"),
        (RegisterARM64::X10, "X10"),
        (RegisterARM64::X11, "X11"),
        (RegisterARM64::X12, "X12"),
        (RegisterARM64::X13, "X13"),
        (RegisterARM64::X14, "X14"),
        (RegisterARM64::X15, "X15"),
        (RegisterARM64::X16, "X16"),
        (RegisterARM64::X17, "X17"),
        (RegisterARM64::X18, "X18"),
        (RegisterARM64::X19, "X19"),
        (RegisterARM64::X20, "X20"),
        (RegisterARM64::X21, "X21"),
        (RegisterARM64::X22, "X22"),
        (RegisterARM64::X23, "X23"),
        (RegisterARM64::X24, "X24"),
        (RegisterARM64::X25, "X25"),
        (RegisterARM64::X26, "X26"),
        (RegisterARM64::X27, "X27"),
        (RegisterARM64::X28, "X28"),
        (RegisterARM64::FP, "FP"),
        (RegisterARM64::LR, "LR"),
        (RegisterARM64::SP, "SP"),
    ];

    let mut line = String::new();
    for (i, (reg, name)) in regs.iter().enumerate() {
        let value = reg_or_zero(uc, *reg);
        let _ = write!(line, " {name}=0x{value:016X}");
        if (i + 1) % 4 == 0 {
            println!("{}", line);
            line = String::new();
        }
    }

    if !line.is_empty() {
        println!("{}", line);
    }
}

