use crate::block;
use crate::block::Block;

pub trait Header {
    type Block: Block;
    type Size;

    const SIZE: isize;

    // fn new(p: *mut u8) -> Self;
    //
    // fn ptr(&self) -> *const u8;
    //
    // fn ptr_mut(&self) -> *mut u8;

    // fn size(&self) -> usize;

    // fn set_size(&mut self, size: usize);

    fn size(p: *const u8) -> usize;
    fn set_size(p: *const u8, size: usize);
}

pub trait Header16 {
}

pub trait Header32 {
}

pub struct Sized16;

impl Header16 for Sized16 {}

impl Header for Sized16 {
    type Block = block::Block16;
    type Size = u16;

    const SIZE: isize = Self::Block::SIZE_BYTES;

    // fn new(p: *mut u8) -> Self {
    //     Header16 { p }
    // }

    // fn ptr(&self) -> *const u8 {
    //     self.p as *const u8
    // }
    //
    // fn ptr_mut(&self) -> *mut u8 {
    //     self.p
    // }

    fn size(p: *const u8) -> usize {
        Self::Block::get_size_value(p)
    }

    fn set_size(p: *const u8, size: usize) {
        Self::Block::put_size_value_usize(unsafe { p as *mut u8 }, size);
    }
}

pub struct Sized32;

impl Header32 for Sized32 {}

impl Header for Sized32 {
    type Block = block::Block32;
    type Size = u32;

    const SIZE: isize = Self::Block::SIZE_BYTES;

    // fn new(p: *mut u8) -> Self {
    //     Header16 { p }
    // }

    // fn ptr(&self) -> *const u8 {
    //     self.p as *const u8
    // }
    //
    // fn ptr_mut(&self) -> *mut u8 {
    //     self.p
    // }

    fn size(p: *const u8) -> usize {
        Self::Block::get_size_value(p)
    }

    fn set_size(p: *const u8, size: usize) {
        Self::Block::put_size_value_usize(unsafe { p as *mut u8 }, size);
    }
}