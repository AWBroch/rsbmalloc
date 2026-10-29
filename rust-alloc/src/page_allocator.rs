use core::{
    alloc::{GlobalAlloc, Layout},
    cmp::min,
    ptr,
};
use lazy_static::lazy_static;
#[cfg(windows)]
use windows_sys::Win32::System::{
    Memory::{VirtualAlloc, VirtualFree, MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_READWRITE},
    SystemInformation::GetSystemInfo,
};

lazy_static! {
    pub static ref PAGE_SIZE: usize = page_size();
}

#[cfg(unix)]
fn page_size() -> usize {
    let size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    if size > 0 {
        size as usize
    } else {
        0
    }
}

#[cfg(windows)]
fn page_size() -> usize {
    unsafe {
        let mut info = core::mem::zeroed();
        GetSystemInfo(&mut info);
        info.dwPageSize as usize
    }
}

// OS mappings guarantee page alignment, not arbitrary Layout alignment.
// Return None if page-size discovery, alignment, or size rounding fails.
fn mapping_layout(layout: Layout) -> Option<Layout> {
    let page_size = *PAGE_SIZE;
    if !page_size.is_power_of_two() || layout.align() > page_size || layout.size() == 0 {
        return None;
    }
    layout.align_to(page_size).ok().map(|l| l.pad_to_align())
}

/// A page-backed allocator accepting alignments up to the OS page size.
/// Unsupported alignments and allocation failures return null.
#[derive(Default)]
pub struct PageAllocator {}

pub static PAGE_ALLOCATOR: PageAllocator = PageAllocator {};

unsafe impl GlobalAlloc for PageAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let aligned_layout = match mapping_layout(layout) {
            Some(l) => l,
            None => return ptr::null_mut(),
        };
        #[cfg(windows)]
        {
            VirtualAlloc(
                ptr::null(),
                aligned_layout.size(),
                MEM_COMMIT | MEM_RESERVE,
                PAGE_READWRITE,
            ) as *mut u8
        }
        #[cfg(unix)]
        {
            let addr = libc::mmap(
                ptr::null_mut(),
                aligned_layout.size(),
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            );
            if addr == libc::MAP_FAILED {
                ptr::null_mut()
            } else {
                addr as *mut u8
            }
        }
    }

    /// Silently fails on OS deallocation errors.
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        #[cfg(windows)]
        {
            let _ = layout;
            // MEM_RELEASE requires the original reservation base and a zero size.
            VirtualFree(ptr as _, 0, MEM_RELEASE);
        }
        #[cfg(unix)]
        if let Some(aligned) = mapping_layout(layout) {
            libc::munmap(ptr as _, aligned.size());
        }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_layout = match Layout::from_size_align(new_size, layout.align()) {
            Ok(l) => l,
            Err(_) => return ptr::null_mut(),
        };
        let old_mapping = match mapping_layout(layout) {
            Some(l) => l,
            None => return ptr::null_mut(),
        };
        let new_mapping = match mapping_layout(new_layout) {
            Some(l) => l,
            None => return ptr::null_mut(),
        };
        if old_mapping.size() == new_mapping.size() {
            return ptr;
        }

        // Keep each allocation in one OS mapping/reservation. On failure the
        // original block remains allocated, with its contents unchanged.
        let new_ptr = self.alloc(new_layout);
        if new_ptr.is_null() {
            return ptr::null_mut();
        }
        ptr::copy_nonoverlapping(ptr, new_ptr, min(layout.size(), new_size));
        self.dealloc(ptr, layout);
        new_ptr
    }
}
