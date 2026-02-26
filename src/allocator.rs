use crate::constants::PAGE_SIZE;
use crate::errors::VmError;
use crate::util::align_up;

#[derive(Debug, Clone)]
pub struct Allocator {
    base: u64,
    size: u64,
    offset: u64,
}

impl Allocator {
    pub fn new(base: u64, size: u64) -> Self {
        Self {
            base,
            size,
            offset: 0,
        }
    }

    pub fn alloc(&mut self, request: u64) -> Result<u64, VmError> {
        let length = align_up(request.max(1), PAGE_SIZE);
        let address = self.base + self.offset;
        let next = self.offset.saturating_add(length);
        if next > self.size {
            return Err(VmError::AllocatorOom {
                base: self.base,
                size: self.size,
                request,
            });
        }
        self.offset = next;
        Ok(address)
    }
}

#[cfg(test)]
mod tests {
    use super::Allocator;

    #[test]
    fn allocator_aligns_to_pages() {
        let mut allocator = Allocator::new(0x1000_0000, 0x20_000);
        let a = allocator.alloc(1).expect("alloc 1");
        let b = allocator.alloc(0x1500).expect("alloc 2");

        assert_eq!(a, 0x1000_0000);
        assert_eq!(b, 0x1000_1000);
    }
}
