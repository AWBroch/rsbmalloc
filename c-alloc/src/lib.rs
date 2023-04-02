#![cfg_attr(not(feature = "std"), no_std)]

use core::{
    alloc::{GlobalAlloc, Layout},
    cmp::max,
    ffi::{c_int, c_void},
    mem, ptr,
};
use rsbmalloc::{page_allocator::PAGE_SIZE, BinnedAlloc};

static ALLOCATOR: BinnedAlloc = BinnedAlloc::new();

fn create_alloc_layout(size: usize) -> Layout {
    Layout::from_size_align(
        size + mem::size_of::<usize>(),
        max(mem::size_of::<usize>(), get_align(size)),
    )
    .unwrap()
}

fn create_calloc_layout(count: usize, size: usize) -> Layout {
    Layout::from_size_align(
        size * count + mem::size_of::<usize>(),
        max(mem::size_of::<usize>(), get_align(size)),
    )
    .unwrap()
}

fn get_align(size: usize) -> usize {
    2_usize.pow(size.ilog2())
}

#[no_mangle]
pub unsafe extern "C" fn malloc(size: usize) -> *mut c_void {
    if size == 0 {
        ptr::null_mut()
    } else {
        let ptr = ALLOCATOR.alloc(create_alloc_layout(size)) as *mut usize;
        ptr.write(size);
        ptr.add(1) as *mut c_void
    }
}

#[no_mangle]
pub unsafe extern "C" fn free(ptr: *mut c_void) {
    if ptr.is_null() {
        return;
    }
    let ptr = (ptr as *mut usize).sub(1);
    let size = ptr.read();
    if size == 0 {
        return;
    } else {
        ALLOCATOR.dealloc(ptr as *mut u8, create_alloc_layout(size));
    }
}

#[no_mangle]
pub unsafe extern "C" fn calloc(count: usize, size: usize) -> *mut c_void {
    let ptr = ALLOCATOR.alloc_zeroed(create_calloc_layout(count, size)) as *mut usize;
    ptr.write(count * size);
    ptr.add(1) as *mut c_void
}

#[no_mangle]
pub unsafe extern "C" fn realloc(ptr: *mut c_void, size: usize) -> *mut c_void {
    if ptr.is_null() {
        malloc(size)
    } else {
        let ptr = (ptr as *mut usize).sub(1);
        let size = ptr.read();
        let layout = create_alloc_layout(size);
        let new_ptr =
            ALLOCATOR.realloc(ptr as *mut u8, layout, size + mem::size_of::<usize>()) as *mut usize;
        new_ptr.write(size);
        new_ptr.add(1) as *mut c_void
    }
}

#[no_mangle]
pub unsafe extern "C" fn aligned_alloc(alignment: usize, size: usize) -> *mut c_void {
    let usize_size = max(mem::size_of::<usize>(), alignment);
    let ptr = ALLOCATOR
        .alloc(Layout::from_size_align_unchecked(
            size + usize_size,
            alignment,
        ))
        .add(usize_size) as *mut usize;
    ptr.write(size);
    ptr.add(1) as *mut c_void
}

#[no_mangle]
pub unsafe extern "C" fn valloc(size: usize) -> *mut c_void {
    aligned_alloc(*PAGE_SIZE, size)
}
#[no_mangle]
pub unsafe extern "C" fn pvalloc(size: usize) -> *mut c_void {
    aligned_alloc(*PAGE_SIZE, size)
}

#[no_mangle]
pub unsafe extern "C" fn memalign(alignment: usize, size: usize) -> *mut c_void {
    aligned_alloc(alignment, size)
}

#[no_mangle]
pub unsafe extern "C" fn posix_memalign(
    memptr: *mut *mut c_void,
    alignment: usize,
    size: usize,
) -> c_int {
    *memptr = aligned_alloc(alignment, size);
    if (*memptr).is_null() {
        return 1;
    } else {
        return 0;
    }
}
