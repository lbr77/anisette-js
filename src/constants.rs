use unicorn_engine::RegisterARM64;

pub const PAGE_SIZE: u64 = 0x1000;

pub const RETURN_ADDRESS: u64 = 0xDEAD_0000;
pub const STACK_ADDRESS: u64 = 0xF000_0000;
pub const STACK_SIZE: u64 = 0x10_0000;

pub const MALLOC_ADDRESS: u64 = 0x6000_0000;
pub const MALLOC_SIZE: u64 = 0x10_00000;

pub const IMPORT_ADDRESS: u64 = 0xA000_0000;
pub const IMPORT_SIZE: u64 = 0x1000;
pub const IMPORT_LIBRARY_STRIDE: u64 = 0x0100_0000;
pub const IMPORT_LIBRARY_COUNT: usize = 10;

pub const TEMP_ALLOC_BASE: u64 = 0x0008_0000_0000;
pub const TEMP_ALLOC_SIZE: u64 = 0x1000_0000;
pub const LIB_ALLOC_BASE: u64 = 0x0010_0000;
pub const LIB_ALLOC_SIZE: u64 = 0x9000_0000;
pub const LIB_RESERVATION_SIZE: u64 = 0x1000_0000;

pub const O_WRONLY: u64 = 0o1;
pub const O_RDWR: u64 = 0o2;
pub const O_ACCMODE: u64 = 0o3;
pub const O_CREAT: u64 = 0o100;
pub const O_NOFOLLOW: u64 = 0o100000;

pub const ENOENT: u32 = 2;

pub const RET_AARCH64: [u8; 4] = [0xC0, 0x03, 0x5F, 0xD6];


pub const ARG_REGS: [RegisterARM64; 29] = [
    RegisterARM64::X0,
    RegisterARM64::X1,
    RegisterARM64::X2,
    RegisterARM64::X3,
    RegisterARM64::X4,
    RegisterARM64::X5,
    RegisterARM64::X6,
    RegisterARM64::X7,
    RegisterARM64::X8,
    RegisterARM64::X9,
    RegisterARM64::X10,
    RegisterARM64::X11,
    RegisterARM64::X12,
    RegisterARM64::X13,
    RegisterARM64::X14,
    RegisterARM64::X15,
    RegisterARM64::X16,
    RegisterARM64::X17,
    RegisterARM64::X18,
    RegisterARM64::X19,
    RegisterARM64::X20,
    RegisterARM64::X21,
    RegisterARM64::X22,
    RegisterARM64::X23,
    RegisterARM64::X24,
    RegisterARM64::X25,
    RegisterARM64::X26,
    RegisterARM64::X27,
    RegisterARM64::X28,
];

pub const DEBUG_PRINT_ENABLED: bool = false;
pub const DEBUG_TRACE_ENABLED: bool = false;
