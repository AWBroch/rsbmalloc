use once_cell::sync::OnceCell;

use crate::*;
use core::ptr;
use std::alloc::GlobalAlloc;

pub(crate) struct ThreadCache {
    pub bins: OnceCell<BinsSlice>,
    fallback: Bins,
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
            fallback: Bins::new(),
        }
    }
    /// Put in any usize, does the modulo-getting
    unsafe fn get_thread_cache(&self, id: usize) -> &Bins {
        let bins_slice = match self.bins.get_or_try_init(init_bins) {
            Ok(bins) => bins,
            // No partially initialized cache is published. Use the embedded
            // synchronized bins and retry cache initialization on a later call.
            Err(()) => return &self.fallback,
        };
        let offset = hash_usize(id) % bins_slice.len;
        // Caches may be shared by colliding thread IDs. Bin mutexes provide
        // interior mutability; a mutable reference to Bins would be unsound.
        &*bins_slice.ptr.add(offset)
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
    unsafe { libc::pthread_self() as usize }
}

#[cfg(windows)]
pub(crate) fn thread_id() -> usize {
    unsafe { windows_sys::Win32::System::Threading::GetCurrentThreadId() as usize }
}

// CPU discovery must not allocate: num_cpus may read cgroup files using heap
// allocations on Linux, recursively entering this cache initializer.
#[cfg(unix)]
fn cpu_count() -> Result<usize, ()> {
    let count = unsafe { libc::sysconf(libc::_SC_NPROCESSORS_ONLN) };
    if count > 0 {
        Ok(count as usize)
    } else {
        Err(())
    }
}

#[cfg(windows)]
fn cpu_count() -> Result<usize, ()> {
    unsafe {
        let mut info = mem::zeroed();
        windows_sys::Win32::System::SystemInformation::GetSystemInfo(&mut info);
        match info.dwNumberOfProcessors {
            0 => Err(()),
            count => Ok(count as usize),
        }
    }
}

fn init_bins() -> Result<BinsSlice, ()> {
    let num_bins = cpu_count()?.checked_mul(4).filter(|&n| n != 0).ok_or(())?;
    let size = num_bins.checked_mul(mem::size_of::<Bins>()).ok_or(())?;
    let layout = Layout::from_size_align(size, mem::align_of::<Bins>()).map_err(|_| ())?;
    unsafe {
        let buf = PAGE_ALLOCATOR.alloc(layout) as *mut Bins;
        if buf.is_null() {
            return Err(());
        }
        for i in 0..num_bins {
            ptr::write(buf.add(i), Bins::new());
        }
        Ok(BinsSlice {
            ptr: buf,
            len: num_bins,
        })
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
        let new_layout = match Layout::from_size_align(new_size, layout.align()) {
            Ok(l) if new_size != 0 => l,
            _ => return ptr::null_mut(),
        };
        if layout.pad_to_align().size() > RSB_CHUNK_SIZE
            && new_layout.pad_to_align().size() > RSB_CHUNK_SIZE
        {
            return PAGE_ALLOCATOR.realloc(ptr, layout, new_size);
        }
        let new_ptr = self.alloc(new_layout);
        if new_ptr.is_null() {
            return ptr::null_mut();
        }
        if new_ptr != ptr {
            core::ptr::copy_nonoverlapping(ptr, new_ptr, min(layout.size(), new_size));
            self.dealloc(ptr, layout);
        }
        new_ptr
    }
}
