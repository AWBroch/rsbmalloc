use core::{
    alloc::{GlobalAlloc, Layout},
    cmp::min,
    ptr,
};
use lazy_static::lazy_static;

lazy_static! {
    pub static ref PAGE_SIZE: usize = page_size();
}

#[cfg(unix)]
fn page_size() -> usize {
    #[cfg(target_os = "macos")]
    unsafe {
        libc::vm_page_size
    }
    #[cfg(not(target_os = "macos"))]
    unsafe {
        libc::sysconf(libc::_SC_PAGESIZE) as usize
    }
}

#[cfg(windows)]
fn page_size() -> usize {
    unsafe {
        let mut info = core::mem::zeroed();
        libc::GetSystemInfo(&mut info);
        return info.dwPageSize as usize;
    }
}

#[derive(Default)]
pub struct PageAllocator {}

pub static PAGE_ALLOCATOR: PageAllocator = PageAllocator {};

unsafe impl GlobalAlloc for PageAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let aligned_layout = match layout.align_to(*PAGE_SIZE) {
            Ok(l) => l.pad_to_align(),
            Err(_) => return ptr::null_mut(),
        };
        #[cfg(windows)]
        {
            let addr = libc::VirtualAlloc(
                ptr::null_mut(),
                aligned_layout.size(),
                libc::MEM_COMMIT | libc::MEM_RESERVE,
                libc::PAGE_READWRITE,
            );
            addr as _
        }
        #[cfg(unix)]
        {
            // let hint = self.next_mmap_addr_hint.load(Ordering::Relaxed);
            let addr = libc::mmap(
                ptr::null_mut(),
                aligned_layout.size(),
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            );
            addr as _
        }
    }

    /// Silently fails on errors
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        #[cfg(windows)]
        libc::VirtualFree(ptr, 0, libc::MEM_RELEASE);
        #[cfg(not(windows))]
        if let Ok(aligned) = layout.align_to(*PAGE_SIZE) {
            libc::munmap(ptr as _, aligned.pad_to_align().size());
        }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let p_size = *PAGE_SIZE;
        let old_aligned_size = match layout.align_to(p_size) {
            Ok(l) => l.pad_to_align(),
            Err(_) => return ptr::null_mut(),
        };
        let aligned_layout = match Layout::from_size_align(new_size, p_size) {
            Ok(l) => l.pad_to_align(),
            Err(_) => return ptr::null_mut(),
        };
        let copy_len = min(layout.size(), new_size);
        #[cfg(windows)]
        {
            if new_size <= old_aligned_size.size() {
                let old_addr_end = ptr.add(old_aligned_size.size());
                let new_addr_end = ptr.add(aligned_layout.size());
                if old_addr_end > new_addr_end {
                    libc::VirtualFree(
                        new_addr_end as _,
                        old_aligned_size.size() - aligned_layout.size(),
                        libc::MEM_DECOMMIT,
                    );
                }
                ptr
            } else {
                let new_addr = libc::VirtualAlloc(
                    ptr::null_mut(),
                    aligned_layout.size(),
                    libc::MEM_COMMIT | libc::MEM_RESERVE,
                    libc::PAGE_READWRITE,
                ) as _;
                ptr::copy_nonoverlapping(ptr, new_addr, layout.size());
                self.dealloc(ptr, layout);
                new_addr
            }
        }
        #[cfg(unix)]
        {
            let old_addr_end = ptr.add(old_aligned_size.size());
            if new_size <= old_aligned_size.size() {
                let new_addr_end = ptr.add(aligned_layout.size());
                if old_addr_end > new_addr_end {
                    libc::munmap(
                        new_addr_end as _,
                        old_aligned_size.size() - aligned_layout.size(),
                    );
                }
                ptr
            } else {
                let appended_addr = libc::mmap(
                    old_addr_end as _,
                    aligned_layout.size() - old_aligned_size.size(),
                    libc::PROT_READ | libc::PROT_WRITE,
                    libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                    -1,
                    0,
                ) as *mut u8;
                if appended_addr == old_addr_end {
                    ptr
                } else {
                    libc::munmap(
                        appended_addr as _,
                        aligned_layout.size() - old_aligned_size.size(),
                    );
                    let new_addr = self.alloc(aligned_layout);

                    ptr::copy_nonoverlapping(ptr, new_addr, copy_len);
                    libc::munmap(ptr as _, old_aligned_size.size());
                    new_addr
                }
            }
        }
    }
}
