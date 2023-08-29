use once_cell::sync::OnceCell;

use crate::*;
use core::ptr;
use std::alloc::GlobalAlloc;

pub(crate) struct ThreadCache {
    pub bins: OnceCell<BinsSlice>,
}

pub(crate) struct BinsSlice {
    pub ptr: *mut Bins,
    pub len: usize,
}

unsafe impl Sync for BinsSlice {}
unsafe impl Send for BinsSlice {}

impl ThreadCache {
    pub(crate) const fn new() -> Self {
        Self {
            bins: OnceCell::new(),
        }
    }
    /// Put in any usize, does the modulo-getting
    unsafe fn get_thread_cache<'a>(&'a self, id: usize) -> &'a mut Bins {
        let bins_slice = self.bins.get_or_init(init_bins);
        let hashed = hash_usize(id);
        let offset = (hashed % bins_slice.len) as isize;
        &mut *bins_slice.ptr.offset(offset)
    }
}

fn hash_usize(input: usize) -> usize {
    let mut output = input as u64;
    output ^= output >> 33;
    output = output.wrapping_mul(0xff51afd7ed558ccd);
    output ^= output >> 33;
    output = output.wrapping_mul(0xc4ceb9fe1a85ec53);
    output ^= output >> 33;
    output as usize
}

#[cfg(unix)]
pub(crate) fn thread_id() -> usize {
    unsafe { libc::pthread_self() }
}

#[cfg(windows)]
pub(crate) fn thread_id() -> usize {
    unsafe { libc::GetCurrentThreadId() as usize }
}

fn init_bins() -> BinsSlice {
    let num_bins = num_cpus::get() * 4;
    unsafe {
        let buf = PAGE_ALLOCATOR.alloc(
            Layout::from_size_align(
                num_bins * Layout::new::<Bins>().pad_to_align().size(),
                mem::align_of::<Bins>(),
            )
            .unwrap(),
        ) as *mut Bins;
        for i in 0..num_bins {
            ptr::write(buf.offset(i as isize), Bins::new());
        }
        BinsSlice {
            ptr: buf,
            len: num_bins,
        }
    }
}

unsafe impl GlobalAlloc for RSBMalloc {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        if layout.align() > MAX_ALIGN {
            return ptr::null_mut();
        }
        let bins = self.thread_cache.get_thread_cache(thread_id());
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
            ..=8192 => bins.bin8192.alloc(),
            ..=16384 => bins.bin16384.alloc(),
            ..=0x8000 => bins.bin32ki.alloc(),
            ..=0x10000 => bins.bin64ki.alloc(),
            _ => PAGE_ALLOCATOR.alloc(layout),
        }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        let bins = self.thread_cache.get_thread_cache(thread_id());
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
            ..=8192 => bins.bin8192.dealloc(ptr),
            ..=16384 => bins.bin16384.dealloc(ptr),
            ..=0x8000 => bins.bin32ki.dealloc(ptr),
            ..=0x10000 => bins.bin64ki.dealloc(ptr),
            _ => PAGE_ALLOCATOR.dealloc(ptr, layout),
        }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if layout.align() > MAX_ALIGN {
            return ptr::null_mut();
        }
        if layout.pad_to_align().size() > RSB_CHUNK_SIZE
            && Layout::from_size_align_unchecked(new_size, layout.align())
                .pad_to_align()
                .size()
                > RSB_CHUNK_SIZE
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
