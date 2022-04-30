use core::ptr;
use std::alloc::{alloc, alloc_zeroed, dealloc, Layout, realloc};
use std::ptr::NonNull;

pub trait Allocator {
    fn allocate(size: usize) -> *mut u8;

    fn deallocate(p: *mut u8, size: usize);
    fn deallocate_layout(p: *mut u8, layout: Layout);

    fn reallocate(p: *mut u8, size: usize) -> (*mut u8, usize);
}

pub struct Global;

impl Allocator for Global {
    fn allocate(size: usize) -> *mut u8 {
        unsafe { alloc_zeroed(Layout::from_size_align_unchecked(size, 1)) }
    }

    fn deallocate(p: *mut u8, size: usize) {
        // unsafe { ptr::drop_in_place(p); }
        unsafe { Box::from_raw(p); }
        // unsafe { std::alloc::Global.deallocate(NonNull::new(p), Layout::from_size_align_unchecked(size, 1)); }
        // unsafe { dealloc(p, Layout::from_size_align_unchecked(size, 1)) }
    }

    fn deallocate_layout(p: *mut u8, layout: Layout) {
        unsafe { dealloc(p, layout) }
    }


    fn reallocate(p: *mut u8, size: usize) -> (*mut u8, usize) {
        unsafe { (realloc(p, Layout::from_size_align_unchecked(size, 1), size), size) }
    }
}

pub struct Borrowed;

impl Allocator for Borrowed {
    fn allocate(size: usize) -> *mut u8 {
        ptr::null_mut()
    }

    fn deallocate(p: *mut u8, size: usize) {}

    fn deallocate_layout(p: *mut u8, layout: Layout) {}

    fn reallocate(p: *mut u8, size: usize) -> (*mut u8, usize) {
        (ptr::null_mut(), 0)
    }
}


pub trait Grow {
    fn calc(current: usize, min: usize) -> usize;
}

pub struct Doubled;

impl Grow for Doubled {
    fn calc(current: usize, min: usize) -> usize {
        core::cmp::max(current * 2, current + min)
    }
}

pub struct Minimal;

impl Grow for Minimal {
    fn calc(current: usize, min: usize) -> usize {
        // pow(2, ceil(log(x)/log(2)));
        current + min
    }
}

pub enum TruncResult {
    Success = 0,
    OutOfMemory = 1,
    Overflow = 2,
    Underflow = 3,
}