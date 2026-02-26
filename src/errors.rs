use thiserror::Error;
use unicorn_engine::unicorn_const::uc_error;

#[derive(Debug, Error)]
pub enum VmError {
    #[error("unicorn error: {0:?}")]
    Unicorn(uc_error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("elf parse error: {0}")]
    Elf(#[from] goblin::error::Error),
    #[error("allocator out of memory: base=0x{base:X} size=0x{size:X} request=0x{request:X}")]
    AllocatorOom { base: u64, size: u64, request: u64 },
    #[error("library not registered: {0}")]
    LibraryNotRegistered(String),
    #[error("library not loaded: {0}")]
    LibraryNotLoaded(usize),
    #[error("symbol not found: {symbol} in {library}")]
    SymbolNotFound { library: String, symbol: String },
    #[error("symbol index out of range: lib={library} index={index}")]
    SymbolIndexOutOfRange { library: String, index: usize },
    #[error("unsupported relocation type: {0}")]
    UnsupportedRelocation(u32),
    #[error("invalid ELF file range")]
    InvalidElfRange,
    #[error("unhandled import: {0}")]
    UnhandledImport(String),
    #[error("invalid import address: 0x{0:X}")]
    InvalidImportAddress(u64),
    #[error("invalid dlopen handle: {0}")]
    InvalidDlopenHandle(u64),
    #[error("invalid file descriptor: {0}")]
    InvalidFileDescriptor(u64),
    #[error("too many cdecl args: {0} (max 29)")]
    TooManyArguments(usize),
    #[error("adi call failed: {name} returned {code}")]
    AdiCallFailed { name: &'static str, code: i32 },
    #[error("unterminated C string at 0x{0:X}")]
    UnterminatedCString(u64),
    #[error("empty path")]
    EmptyPath,
    #[error("integer conversion failed for value: {0}")]
    IntegerOverflow(u64),
}

impl From<uc_error> for VmError {
    fn from(value: uc_error) -> Self {
        Self::Unicorn(value)
    }
}
