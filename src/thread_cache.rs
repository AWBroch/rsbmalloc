use crate::*;
use core::{
    cell::UnsafeCell,
    hash::{Hash, Hasher},
    num::Wrapping,
    ptr,
};
use std::{alloc::GlobalAlloc, collections::hash_map::DefaultHasher, sync::Once};

static CREATE_BINS: Once = Once::new();
pub(crate) static THREAD_CACHE: ThreadCache = ThreadCache::new();

pub(crate) struct ThreadCache {
    pub bins: UnsafeCell<BinsSlice>,
}

unsafe impl Sync for ThreadCache {}

pub(crate) struct BinsSlice {
    pub ptr: *mut Bins,
    pub len: usize,
}

impl ThreadCache {
    const fn new() -> Self {
        Self {
            bins: UnsafeCell::new(BinsSlice {
                ptr: ptr::null_mut(),
                len: 0,
            }),
        }
    }
    /// Put in any usize, does the modulo-getting
    unsafe fn get_thread_cache(&self, id: usize) -> Bins {
        let bins_slice = self.bins.get().read();
        let mut hasher = DefaultHasher::new();
        id.hash(&mut hasher);
        bins_slice
            .ptr
            .offset((hasher.finish() as usize % bins_slice.len) as isize)
            .read()
    }
}

pub(crate) struct SimpleHasher(pub(crate) Wrapping<u64>);

impl Hasher for SimpleHasher {
    fn finish(&self) -> u64 {
        self.0 .0
    }
    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 += *byte as u64;
        }
    }
}

#[cfg(unix)]
pub(crate) fn thread_id() -> usize {
    unsafe { libc::pthread_self() }
}

#[cfg(windows)]
pub(crate) fn thread_id() -> usize {
    unsafe { libc::GetCurrentThreadId() as usize }
}

fn init_bins() {
    let num_bins = num_cpus::get();
    unsafe {
        let mut buf = PAGE_ALLOCATOR.alloc(
            Layout::from_size_align(
                num_bins * Layout::new::<Bins>().pad_to_align().size(),
                mem::align_of::<Bins>(),
            )
            .unwrap(),
        ) as *mut Bins;
        for _ in 0..num_bins {
            ptr::write(buf, Bins::new());
            buf = buf.offset(1);
        }
        THREAD_CACHE.bins.get().write(BinsSlice {
            ptr: buf,
            len: num_bins,
        });
    }
}

unsafe impl GlobalAlloc for BinnedAlloc {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        CREATE_BINS.call_once(init_bins);
        let bins = THREAD_CACHE.get_thread_cache(thread_id());
        let size = layout.pad_to_align().size();
        match size {
            ..=4 => bins.bin4.alloc(),
            ..=8 => bins.bin8.alloc(),
            ..=16 => bins.bin16.alloc(),
            ..=32 => bins.bin32.alloc(),
            ..=64 => bins.bin64.alloc(),
            ..=128 => bins.bin128.alloc(),
            ..=256 => bins.bin256.alloc(),
            ..=512 => bins.bin512.alloc(),
            ..=1024 => bins.bin1024.alloc(),
            ..=2048 => bins.bin2048.alloc(),
            ..=4096 => bins.bin4096.alloc(),
            _ => PAGE_ALLOCATOR.alloc(layout),
        }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        CREATE_BINS.call_once(init_bins);
        let bins = THREAD_CACHE.get_thread_cache(thread_id());
        let size = layout.pad_to_align().size();
        match size {
            ..=4 => bins.bin4.dealloc(ptr),
            ..=8 => bins.bin8.dealloc(ptr),
            ..=16 => bins.bin16.dealloc(ptr),
            ..=32 => bins.bin32.dealloc(ptr),
            ..=64 => bins.bin64.dealloc(ptr),
            ..=128 => bins.bin128.dealloc(ptr),
            ..=256 => bins.bin256.dealloc(ptr),
            ..=512 => bins.bin512.dealloc(ptr),
            ..=1024 => bins.bin1024.dealloc(ptr),
            ..=2048 => bins.bin2048.dealloc(ptr),
            ..=4096 => bins.bin4096.dealloc(ptr),
            _ => PAGE_ALLOCATOR.dealloc(ptr, layout),
        }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if layout.pad_to_align().size() > 4096
            && Layout::from_size_align_unchecked(new_size, layout.align())
                .pad_to_align()
                .size()
                > 4096
        {
            return PAGE_ALLOCATOR.realloc(ptr, layout, new_size);
        }
        let new_ptr = self.alloc(Layout::from_size_align_unchecked(new_size, layout.align()));
        if new_ptr != ptr {
            core::ptr::copy_nonoverlapping(ptr, new_ptr, min(layout.size(), new_size));
            self.dealloc(ptr, layout);
        }
        new_ptr
    }
}
