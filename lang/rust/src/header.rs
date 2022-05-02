use std::marker::PhantomData;
use std::mem;

use crate::block;
use crate::block::Block;

pub trait Header: Sized {
    type Block: Block;
    type Size;

    const SIZE: isize;

    fn size(&self) -> usize;

    fn set_size(&mut self, size: usize) -> &mut Self;

    fn base_size(&self) -> usize;

    fn set_base_size(&mut self, size: usize) -> &mut Self;

    fn raw_size(p: *const u8) -> usize;

    fn raw_set_size(p: *const u8, size: usize);

    fn init(p: *mut u8, size: usize, base_size: usize) -> *mut u8 {
        (unsafe { (&mut *(p as *mut Self)) }).set_size(size).set_base_size(base_size);
        p
    }
}

pub struct Fixed<T: Sized> {
    _phantom: PhantomData<T>,
}

pub trait Header16 {}

pub trait Header32 {}

pub trait Header64 {}

#[repr(C, packed)]
pub struct Fixed16 {
    size: u16,
}

impl Header16 for Fixed16 {}

impl Header for Fixed16 {
    type Block = block::Block16;
    type Size = u16;

    const SIZE: isize = mem::size_of::<Self>() as isize;

    fn size(&self) -> usize {
        u16::from_le(self.size.to_le()) as usize
    }

    fn set_size(&mut self, size: usize) -> &mut Self {
        self.size = size.to_le() as u16;
        self
    }

    fn base_size(&self) -> usize {
        self.size()
    }

    fn set_base_size(&mut self, size: usize) -> &mut Self {
        self.set_size(size);
        self
    }

    fn raw_size(p: *const u8) -> usize {
        Self::Block::get_size_value(p)
    }

    fn raw_set_size(p: *const u8, size: usize) {
        Self::Block::put_size_value_usize(unsafe { p as *mut u8 }, size);
    }
}

#[repr(C, packed)]
pub struct Flex16 {
    size: u16,
    base_size: u16,
}

impl Header16 for Flex16 {}

impl Header for Flex16 {
    type Block = block::Block16;
    type Size = u16;

    const SIZE: isize = mem::size_of::<Self>() as isize;

    fn size(&self) -> usize {
        u16::from_le(self.size.to_le()) as usize
    }

    fn set_size(&mut self, size: usize) -> &mut Self {
        self.size = size.to_le() as u16;
        self
    }

    fn base_size(&self) -> usize {
        self.size()
    }

    fn set_base_size(&mut self, size: usize) -> &mut Self {
        self.set_size(size);
        self
    }

    fn raw_size(p: *const u8) -> usize {
        Self::Block::get_size_value(p)
    }

    fn raw_set_size(p: *const u8, size: usize) {
        Self::Block::put_size_value_usize(unsafe { p as *mut u8 }, size);
    }
}

#[repr(C, packed)]
pub struct Fixed32 {
    size: u32,
}

impl Header32 for Fixed32 {}

impl Header for Fixed32 {
    type Block = block::Block32;
    type Size = u32;

    const SIZE: isize = Self::Block::SIZE_BYTES;

    fn size(&self) -> usize {
        u32::from_le(self.size) as usize
    }

    fn set_size(&mut self, size: usize) -> &mut Self {
        self.size = size.to_le() as u32;
        self
    }

    fn base_size(&self) -> usize {
        self.size()
    }

    fn set_base_size(&mut self, size: usize) -> &mut Self {
        self.set_size(size);
        self
    }

    fn raw_size(p: *const u8) -> usize {
        Self::Block::get_size_value(p)
    }

    fn raw_set_size(p: *const u8, size: usize) {
        Self::Block::put_size_value_usize(unsafe { p as *mut u8 }, size);
    }
}

#[repr(C, packed)]
pub struct Flex32 {
    size: u32,
    base_size: u32,
}

impl Header for Flex32 {
    type Block = block::Block32;
    type Size = u16;

    const SIZE: isize = mem::size_of::<Self>() as isize;

    fn size(&self) -> usize {
        u32::from_le(self.size.to_le()) as usize
    }

    fn set_size(&mut self, size: usize) -> &mut Self {
        self.size = size.to_le() as u32;
        self
    }

    fn base_size(&self) -> usize {
        u32::from_le(self.base_size.to_le()) as usize
    }

    fn set_base_size(&mut self, size: usize) -> &mut Self {
        self.base_size = size.to_le() as u32;
        self
    }

    fn raw_size(p: *const u8) -> usize {
        Self::Block::get_size_value(p)
    }

    fn raw_set_size(p: *const u8, size: usize) {
        Self::Block::put_size_value_usize(unsafe { p as *mut u8 }, size);
    }
}

#[repr(C, packed)]
pub struct Sized64 {
    size: u64,
}
