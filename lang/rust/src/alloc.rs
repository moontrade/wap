use std::alloc::{alloc, dealloc, Layout, realloc};
use core::ptr;

pub trait Allocator {
    fn allocate(size: usize) -> *mut u8;

    fn deallocate(p: *mut u8, size: usize);

    fn reallocate(p: *mut u8, size: usize) -> (*mut u8, usize);
}

pub struct Global;

impl Allocator for Global {
    fn allocate(size: usize) -> *mut u8 {
        unsafe { alloc(Layout::from_size_align_unchecked(size, 1)) }
    }

    fn deallocate(p: *mut u8, size: usize) {
        unsafe { dealloc(p, Layout::from_size_align_unchecked(size, 1)) }
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

    fn reallocate(p: *mut u8, size: usize) -> (*mut u8, usize) {
        (ptr::null_mut(), 0)
    }
}
