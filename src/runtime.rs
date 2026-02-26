use std::collections::HashMap;
use std::fs::File;

use crate::allocator::Allocator;
use crate::constants::{
    LIB_ALLOC_BASE, LIB_ALLOC_SIZE, MALLOC_ADDRESS, MALLOC_SIZE, TEMP_ALLOC_BASE, TEMP_ALLOC_SIZE,
};

#[derive(Debug, Clone)]
pub(crate) struct SymbolEntry {
    pub(crate) name: String,
    pub(crate) resolved: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct LoadedLibrary {
    pub(crate) name: String,
    pub(crate) symbols: Vec<SymbolEntry>,
    pub(crate) symbols_by_name: HashMap<String, u64>,
}

#[derive(Debug)]
pub(crate) struct RuntimeState {
    pub(crate) temp_allocator: Allocator,
    pub(crate) library_allocator: Allocator,
    pub(crate) malloc_allocator: Allocator,
    pub(crate) errno_address: Option<u64>,
    pub(crate) library_blobs: HashMap<String, Vec<u8>>,
    pub(crate) loaded_libraries: Vec<LoadedLibrary>,
    pub(crate) file_handles: Vec<Option<File>>,
    pub(crate) library_root: Option<String>,
}

impl RuntimeState {
    pub(crate) fn new() -> Self {
        Self {
            temp_allocator: Allocator::new(TEMP_ALLOC_BASE, TEMP_ALLOC_SIZE),
            library_allocator: Allocator::new(LIB_ALLOC_BASE, LIB_ALLOC_SIZE),
            malloc_allocator: Allocator::new(MALLOC_ADDRESS, MALLOC_SIZE),
            errno_address: None,
            library_blobs: HashMap::new(),
            loaded_libraries: Vec::new(),
            file_handles: Vec::new(),
            library_root: None,
        }
    }
}
