pub mod bump;

use alloc::alloc::{GlobalAlloc, Layout};
use bump::BumpAllocator;
use core::ptr::null_mut;

use x86_64::{
    structures::paging::{
        mapper::MapToError, page::PageRangeInclusive, FrameAllocator, Mapper, Page, PageTableFlags,
        Size4KiB,
    },
    VirtAddr,
};

#[global_allocator]
static ALLOCATOR: Locked<BumpAllocator> = Locked::new(BumpAllocator::new());

pub struct Locked<T> {
    inner: spin::Mutex<T>,
}

impl<T> Locked<T> {
    pub const fn new(inner: T) -> Self {
        Locked {
            inner: spin::Mutex::new(inner),
        }
    }

    pub fn lock(&self) -> spin::MutexGuard<T> {
        self.inner.lock()
    }
}

fn align_up(addr: usize, align: usize) -> usize {
    let remainder = addr % align;

    if remainder == 0 {
        addr
    } else {
        addr - remainder + align
    }
}

// Requires that `align` is a power of 2. Then if e.g. `align` is 4K, then:
//
//      align           = 0b0001000000000000
//      align - 1       = 0b0000111111111111
//      !(align - 1)    = 0b1111000000000000
//
// So the `&` drops low bits from the `addr` so it becomes a multiple of `align`
//
fn _align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}

pub const HEAP_START: usize = 0x_4444_4444_0000;
pub const HEAP_SIZE: usize = 100 * 1024; // 100 KiB

pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    for page in page_range() {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;

        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

        unsafe {
            mapper.map_to(page, frame, flags, frame_allocator)?.flush();
        }
    }

    unsafe {
        ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE);
    }

    Ok(())
}

fn page_range() -> PageRangeInclusive {
    let heap_start = VirtAddr::new(HEAP_START as u64);
    let heap_start_page = Page::containing_address(heap_start);

    let heap_end = heap_start + HEAP_SIZE - 1u64;
    let heap_end_page = Page::containing_address(heap_end);

    Page::range_inclusive(heap_start_page, heap_end_page)
}

pub struct Dummy;

unsafe impl GlobalAlloc for Dummy {
    unsafe fn alloc(&self, _layout: Layout) -> *mut u8 {
        null_mut()
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        panic!("dealloc should never be called");
    }
}
