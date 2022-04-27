use std::marker::PhantomData;
use std::mem::ManuallyDrop;
use std::ptr;

pub trait Block {
    type Size;

    // The size of the block header exposed to used blocks is the size field.
    // The prev_phys field is stored *inside* the previous free block.
    const OVERHEAD: isize;
    const DATA_OFFSET: isize;
    const MIN_SIZE: isize;

    const SIZE_LIMIT: usize;
    const SIZE_BYTES: isize;
    const SIZE_BYTES_USIZE: usize;

    fn size(&self) -> Self::Size;
    fn size_usize(&self) -> usize;
    fn prev_phys(&self) -> Self::Size;
    fn next_free(&self) -> Self::Size;
    fn prev_free(&self) -> Self::Size;
    fn is_free(&self) -> bool;
    fn is_prev_free(&self) -> bool;

    fn set_size(&mut self, size: Self::Size);
    fn set_size_usize(&mut self, size: usize);
    fn set_used(&mut self);
    fn set_free(&mut self);
    fn set_prev_free(&mut self);
    fn set_prev_used(&mut self);

    fn offset(base: *const u8, block: *mut Self) -> isize;
    fn to_ptr(&self) -> *const u8;
    fn data_ptr(&self) -> *const u8;
    fn from_ptr<'s>(p: *const u8) -> &'s mut Self;

    fn get_size_value(p: *const u8) -> usize;
    fn get_size_value_mut(p: *mut u8) -> usize;
    fn put_size_value(p: *mut u8, size: Self::Size);
    fn put_size_value_usize(p: *mut u8, size: usize);
}

struct Block_<S> {
    prev_phys: S,
    size: S,
    next_free: S,
    prev_free: S,
}

impl<S> Block_<S> {
    // Since block sizes are always at least a multiple of 4, the two least
    // significant bits of the size field are used to store the block status:
    // - bit 0: whether block is used or free
    // - bit 1: whether previous block is used or free
    // const FREE_BIT: S = S::shift_left(S::from(1), S::from(0));
    // const PREV_FREE_BIT: S = S::shift_left(S::from(1), S::from(1));
}

pub struct Block16 {
    prev_phys: u16,
    size: u16,
    next_free: u16,
    prev_free: u16,
}

pub struct Block32 {
    prev_phys: u32,
    size: u32,
    next_free: u32,
    prev_free: u32,
}

pub struct Block64 {
    prev_phys: u64,
    size: u64,
    next_free: u64,
    prev_free: u64,
}

impl Block16 {
    // Since block sizes are always at least a multiple of 4, the two least
    // significant bits of the size field are used to store the block status:
    // - bit 0: whether block is used or free
    // - bit 1: whether previous block is used or free
    const FREE_BIT: u16 = 1 << 0;
    const PREV_FREE_BIT: u16 = 1 << 1;
    const CLEAR: u16 = !(Self::FREE_BIT | Self::PREV_FREE_BIT);
}

impl Block for Block16 {
    type Size = u16;


    // The size of the block header exposed to used blocks is the size field.
    // The prev_phys field is stored *inside* the previous free block.
    const OVERHEAD: isize = core::mem::size_of::<Self::Size>() as isize;
    const DATA_OFFSET: isize = (core::mem::size_of::<Self::Size>() * 2) as isize;
    const MIN_SIZE: isize = (core::mem::size_of::<Self::Size>() * 3) as isize;

    const SIZE_LIMIT: usize = u16::MAX as usize;


    const SIZE_BYTES: isize = 2;
    const SIZE_BYTES_USIZE: usize = 2;

    #[inline(always)]
    fn size(&self) -> u16 {
        self.size & !(Block16::FREE_BIT | Block16::PREV_FREE_BIT)
    }

    #[inline(always)]
    fn size_usize(&self) -> usize {
        self.size() as usize
    }

    #[inline(always)]
    fn prev_phys(&self) -> u16 {
        self.prev_phys
    }

    #[inline(always)]
    fn next_free(&self) -> u16 {
        self.next_free
    }

    #[inline(always)]
    fn prev_free(&self) -> u16 {
        self.prev_free
    }

    #[inline(always)]
    fn is_free(&self) -> bool {
        self.size & Block16::FREE_BIT != 0
    }

    #[inline(always)]
    fn is_prev_free(&self) -> bool {
        self.size & Block16::PREV_FREE_BIT != 0
    }

    #[inline(always)]
    fn set_size(&mut self, size: u16) {
        self.size = size | (self.size & (Block16::FREE_BIT | Block16::PREV_FREE_BIT));
    }

    #[inline(always)]
    fn set_size_usize(&mut self, size: usize) {
        self.set_size(size as u16);
    }

    #[inline(always)]
    fn set_used(&mut self) {
        self.size &= !Block16::FREE_BIT;
    }

    #[inline(always)]
    fn set_free(&mut self) {
        self.size |= Block16::FREE_BIT;
    }

    #[inline(always)]
    fn set_prev_free(&mut self) {
        self.size |= Block16::PREV_FREE_BIT;
    }

    #[inline(always)]
    fn set_prev_used(&mut self) {
        self.size &= !Block16::PREV_FREE_BIT;
    }

    #[inline(always)]
    fn offset(base: *const u8, block: *mut Self) -> isize {
        unsafe { (block as *mut u8).offset(Self::OVERHEAD) as isize - base as isize }
    }

    #[inline(always)]
    fn to_ptr(&self) -> *const u8 {
        unsafe { (self as *const Self as *const u8).offset(Self::SIZE_BYTES) }
    }

    #[inline(always)]
    fn data_ptr(&self) -> *const u8 {
        unsafe { (self as *const Self as *const u8).offset(Self::DATA_OFFSET) }
    }

    #[inline(always)]
    fn from_ptr<'s>(p: *const u8) -> &'s mut Self {
        unsafe { &mut *(p.offset(-Self::SIZE_BYTES) as *mut u8 as *mut Self) }
    }

    #[inline(always)]
    fn get_size_value(p: *const u8) -> usize {
        unsafe { Self::Size::from_le(*(p as *const Self::Size)) as usize }
    }

    #[inline(always)]
    fn get_size_value_mut(p: *mut u8) -> usize {
        unsafe { Self::Size::from_le(*(p as *const Self::Size)) as usize }
    }

    #[inline(always)]
    fn put_size_value(p: *mut u8, size: Self::Size) {
        unsafe { *(p as *mut Self::Size) = size.to_le(); }
    }

    #[inline(always)]
    fn put_size_value_usize(p: *mut u8, size: usize) {
        unsafe { *(p as *mut Self::Size) = (size as Self::Size).to_le(); }
    }
}

impl Block32 {
    // Since block sizes are always at least a multiple of 4, the two least
    // significant bits of the size field are used to store the block status:
    // - bit 0: whether block is used or free
    // - bit 1: whether previous block is used or free
    const FREE_BIT: u32 = 1 << 0;
    const PREV_FREE_BIT: u32 = 1 << 1;
    const CLEAR: u32 = !(Self::FREE_BIT | Self::PREV_FREE_BIT);
}

impl Block for Block32 {
    type Size = u32;

    // The size of the block header exposed to used blocks is the size field.
    // The prev_phys field is stored *inside* the previous free block.
    const OVERHEAD: isize = core::mem::size_of::<u32>() as isize;
    const DATA_OFFSET: isize = (core::mem::size_of::<u32>() * 2) as isize;
    const MIN_SIZE: isize = (core::mem::size_of::<Self::Size>() * 3) as isize;

    const SIZE_LIMIT: usize = u32::MAX as usize;
    const SIZE_BYTES: isize = 4;
    const SIZE_BYTES_USIZE: usize = 4;

    #[inline(always)]
    fn size(&self) -> u32 {
        self.size & !(Block32::FREE_BIT | Block32::PREV_FREE_BIT)
    }

    #[inline(always)]
    fn size_usize(&self) -> usize {
        self.size() as usize
    }

    #[inline(always)]
    fn prev_phys(&self) -> u32 {
        self.prev_phys
    }

    #[inline(always)]
    fn next_free(&self) -> u32 {
        self.next_free
    }

    #[inline(always)]
    fn prev_free(&self) -> u32 {
        self.prev_free
    }

    #[inline(always)]
    fn is_free(&self) -> bool {
        self.size & Block32::FREE_BIT != 0
    }

    #[inline(always)]
    fn is_prev_free(&self) -> bool {
        self.size & Block32::PREV_FREE_BIT != 0
    }

    #[inline(always)]
    fn set_size(&mut self, size: u32) {
        self.size = size | (self.size & (Block32::FREE_BIT | Block32::PREV_FREE_BIT));
    }

    #[inline(always)]
    fn set_size_usize(&mut self, size: usize) {
        self.set_size(size as u32);
    }

    #[inline(always)]
    fn set_used(&mut self) {
        self.size &= !Block32::FREE_BIT;
    }

    #[inline(always)]
    fn set_free(&mut self) {
        self.size |= Block32::FREE_BIT;
    }

    #[inline(always)]
    fn set_prev_free(&mut self) {
        self.size |= Block32::PREV_FREE_BIT;
    }

    #[inline(always)]
    fn set_prev_used(&mut self) {
        self.size &= !Block32::PREV_FREE_BIT;
    }

    #[inline(always)]
    fn offset(base: *const u8, block: *mut Self) -> isize {
        unsafe { (block as *mut u8).offset(Self::OVERHEAD) as isize - base as isize }
    }

    #[inline(always)]
    fn to_ptr(&self) -> *const u8 {
        unsafe { (self as *const Self as *const u8).offset(Self::SIZE_BYTES) }
    }

    #[inline(always)]
    fn data_ptr(&self) -> *const u8 {
        unsafe { (self as *const Self as *const u8).offset(Self::DATA_OFFSET) }
    }

    #[inline(always)]
    fn from_ptr<'s>(p: *const u8) -> &'s mut Self {
        unsafe { &mut *(p.offset(-Self::SIZE_BYTES) as *mut u8 as *mut Self) }
    }

    #[inline(always)]
    fn get_size_value(p: *const u8) -> usize {
        unsafe { Self::Size::from_le(*(p as *const Self::Size)) as usize }
    }

    #[inline(always)]
    fn get_size_value_mut(p: *mut u8) -> usize {
        unsafe { Self::Size::from_le(*(p as *const Self::Size)) as usize }
    }

    #[inline(always)]
    fn put_size_value(p: *mut u8, size: Self::Size) {
        unsafe { *(p as *mut Self::Size) = size.to_le(); }
    }

    #[inline(always)]
    fn put_size_value_usize(p: *mut u8, size: usize) {
        unsafe { *(p as *mut Self::Size) = (size as Self::Size).to_le(); }
    }
}
//
// pub trait Slice {
//     type Block;
//     type Size;
//
//     fn ptr(&self) -> *const u8;
//
//     fn ptr_mut(&mut self) -> *mut u8;
//
//     fn bool(&self, offset: isize) -> bool;
//     fn set_bool(&mut self, offset: isize, value: bool);
//
//     fn i8(&self, offset: isize) -> i8;
//     fn set_i8(&mut self, offset: isize, value: i8);
//
//     fn u8(&self, offset: isize) -> u8;
//     fn set_u8(&mut self, offset: isize, value: u8);
//
//     fn i16le(&self, offset: isize) -> i16;
//     fn set_i16le(&mut self, offset: isize, value: i16);
//     fn i16be(&self, offset: isize) -> i16;
//     fn set_i16be(&mut self, offset: isize, value: i16);
//     fn i16ne(&self, offset: isize) -> i16;
//     fn set_i16ne(&mut self, offset: isize, value: i16);
//
//     fn i32le(&self, offset: isize) -> i32;
//     fn set_i32le(&mut self, offset: isize, value: i32);
//     fn i32be(&self, offset: isize) -> i32;
//     fn set_i32be(&mut self, offset: isize, value: i32);
//     fn i32ne(&self, offset: isize) -> i32;
//     fn set_i32ne(&mut self, offset: isize, value: i32);
//
//     fn i64le(&self, offset: isize) -> i64;
//     fn set_i64le(&mut self, offset: isize, value: i64);
//     fn i64be(&self, offset: isize) -> i64;
//     fn set_i64be(&mut self, offset: isize, value: i64);
//     fn i64ne(&self, offset: isize) -> i64;
//     fn set_i64ne(&mut self, offset: isize, value: i64);
//
//     fn i128le(&self, offset: isize) -> i128;
//     fn set_i128le(&mut self, offset: isize, value: i128);
//     fn i128be(&self, offset: isize) -> i128;
//     fn set_i128be(&mut self, offset: isize, value: i128);
//     fn i128ne(&self, offset: isize) -> i128;
//     fn set_i128ne(&mut self, offset: isize, value: i128);
//
//     fn u16le(&self, offset: isize) -> u16;
//     fn set_u16le(&mut self, offset: isize, value: u16);
//     fn u16be(&self, offset: isize) -> u16;
//     fn set_u16be(&mut self, offset: isize, value: u16);
//     fn u16ne(&self, offset: isize) -> u16;
//     fn set_u16ne(&mut self, offset: isize, value: u16);
//
//     fn u32le(&self, offset: isize) -> u32;
//     fn set_u32le(&mut self, offset: isize, value: u32);
//     fn u32be(&self, offset: isize) -> u32;
//     fn set_u32be(&mut self, offset: isize, value: u32);
//     fn u32ne(&self, offset: isize) -> u32;
//     fn set_u32ne(&mut self, offset: isize, value: u32);
//
//     fn u64le(&self, offset: isize) -> u64;
//     fn set_u64le(&mut self, offset: isize, value: u64);
//     fn u64be(&self, offset: isize) -> u64;
//     fn set_u64be(&mut self, offset: isize, value: u64);
//     fn u64ne(&self, offset: isize) -> u64;
//     fn set_u64ne(&mut self, offset: isize, value: u64);
//
//     fn u128le(&self, offset: isize) -> u128;
//     fn set_u128le(&mut self, offset: isize, value: u128);
//     fn u128be(&self, offset: isize) -> u128;
//     fn set_u128be(&mut self, offset: isize, value: u128);
//     fn u128ne(&self, offset: isize) -> u128;
//     fn set_u128ne(&mut self, offset: isize, value: u128);
//
//     fn f32le(&self, offset: isize) -> f32;
//     fn set_f32le(&mut self, offset: isize, value: f32);
//     fn f32be(&self, offset: isize) -> f32;
//     fn set_f32be(&mut self, offset: isize, value: f32);
//     fn f32ne(&self, offset: isize) -> f32;
//     fn set_f32ne(&mut self, offset: isize, value: f32);
//
//     fn f64le(&self, offset: isize) -> f64;
//     fn set_f64le(&mut self, offset: isize, value: f64);
//     fn f64be(&self, offset: isize) -> f64;
//     fn set_f64be(&mut self, offset: isize, value: f64);
//     fn f64ne(&self, offset: isize) -> f64;
//     fn set_f64ne(&mut self, offset: isize, value: f64);
//
//     fn str(&self, offset: isize, capacity: usize) -> &str;
//     fn str_slice(&self, offset: isize, capacity: usize) -> &[u8];
//
//     fn put_str(&mut self, offset: isize, capacity: usize, value: &str);
//     fn put_str_slice(&mut self, offset: isize, capacity: usize, value: &[u8]);
//
//     fn slice(&self, offset: isize, capacity: usize) -> &[u8];
//     fn slice_mut(&mut self, offset: isize, capacity: usize) -> &mut [u8];
// }
//
// pub trait Message {
//     type Block: Block;
//     type Size;
//     type Record;
//
//     fn get(&self) -> &Self;
//
//     fn base(&self) -> *mut u8;
//
//     fn tail(&self) -> *mut u8;
// }
//
// // Fixed contains no pointer fields.
// pub enum Fixed<M: Message> {
//     Safe(FixedSafe<M>),
//
//     Unsafe(FixedUnsafe<M>),
// }
//
// impl<M: Message> FixedSafe<M> {
//     fn new(b: *mut u8) -> Self {
//         FixedSafe { b, _block: PhantomData }
//     }
// }
//
// // Trusted buffer has no bounds checking
// pub struct FixedSafe<M: Message> {
//     b: *mut u8,
//     _block: PhantomData<M>,
// }
//
// impl<M: Message> FixedUnsafe<M> {
//     pub fn new(b: *mut u8, end: *mut u8) -> Self {
//         FixedUnsafe { b, end, _block: PhantomData }
//     }
// }
//
// // pub type Fixed16 = FixedSafe<Block16>;
// // pub type Fixed32 = FixedSafe<Block32>;
//
// pub struct FixedUnsafe<M: Message> {
//     //
//     b: *mut u8,
//     //
//     end: *mut u8,
//     _block: PhantomData<M>,
// }
//
// // pub type FixedUnsafe16 = FixedUnsafe<Block16>;
// // pub type FixedUnsafe32 = FixedUnsafe<Block32>;
//
// impl<M: Message> Slice for FixedSafe<M> {
//     type Block = M::Block;
//     type Size = M::Size;
//
//     #[inline(always)]
//     fn ptr(&self) -> *const u8 {
//         self.b
//     }
//
//     #[inline(always)]
//     fn ptr_mut(&mut self) -> *mut u8 {
//         self.b
//     }
//
//     ////////////////////////////////////////////////////////////////
//     // bool
//     ////////////////////////////////////////////////////////////////
//
//     #[inline(always)]
//     fn bool(&self, offset: isize) -> bool {
//         unsafe { *(self.b.offset(offset)) != 0 }
//     }
//     #[inline(always)]
//     fn set_bool(&mut self, offset: isize, value: bool) {
//         unsafe { *(self.b.offset(offset)) = if value { 1 } else { 0 }; }
//     }
//
//     ////////////////////////////////////////////////////////////////
//     // i8
//     ////////////////////////////////////////////////////////////////
//     #[inline(always)]
//     fn i8(&self, offset: isize) -> i8 {
//         unsafe { *(self.b.offset(offset) as *const i8) }
//     }
//     #[inline(always)]
//     fn set_i8(&mut self, offset: isize, value: i8) {
//         unsafe { *(self.b.offset(offset) as *mut i8) = value; }
//     }
//
//     ////////////////////////////////////////////////////////////////
//     // u8
//     ////////////////////////////////////////////////////////////////
//
//     #[inline(always)]
//     fn u8(&self, offset: isize) -> u8 {
//         unsafe { *(self.b.offset(offset)) }
//     }
//     #[inline(always)]
//     fn set_u8(&mut self, offset: isize, value: u8) {
//         unsafe { *(self.b.offset(offset)) = value; }
//     }
//
//     ////////////////////////////////////////////////////////////////
//     // i16
//     ////////////////////////////////////////////////////////////////
//
//     #[inline(always)]
//     fn i16le(&self, offset: isize) -> i16 {
//         unsafe { i16::from_le(*(self.b.offset(offset) as *const i16)) }
//     }
//     #[inline(always)]
//     fn set_i16le(&mut self, offset: isize, value: i16) {
//         unsafe { *(self.b.offset(offset) as *mut i16) = value.to_le(); }
//     }
//     #[inline(always)]
//     fn i16be(&self, offset: isize) -> i16 {
//         unsafe { i16::from_be(*(self.b.offset(offset) as *const i16)) }
//     }
//     #[inline(always)]
//     fn set_i16be(&mut self, offset: isize, value: i16) {
//         unsafe { *(self.b.offset(offset) as *mut i16) = value.to_be(); }
//     }
//     #[inline(always)]
//     fn i16ne(&self, offset: isize) -> i16 {
//         unsafe { *(self.b.offset(offset) as *const i16) }
//     }
//     #[inline(always)]
//     fn set_i16ne(&mut self, offset: isize, value: i16) {
//         unsafe { *(self.b.offset(offset) as *mut i16) = value; }
//     }
//
//     ////////////////////////////////////////////////////////////////
//     // i32
//     ////////////////////////////////////////////////////////////////
//
//     #[inline(always)]
//     fn i32le(&self, offset: isize) -> i32 {
//         unsafe { i32::from_le(*(self.b.offset(offset) as *const i32)) }
//     }
//     #[inline(always)]
//     fn set_i32le(&mut self, offset: isize, value: i32) {
//         unsafe { *(self.b.offset(offset) as *mut i32) = value.to_le(); }
//     }
//     #[inline(always)]
//     fn i32be(&self, offset: isize) -> i32 {
//         unsafe { i32::from_be(*(self.b.offset(offset) as *const i32)) }
//     }
//     #[inline(always)]
//     fn set_i32be(&mut self, offset: isize, value: i32) {
//         unsafe { *(self.b.offset(offset) as *mut i32) = value.to_be(); }
//     }
//     #[inline(always)]
//     fn i32ne(&self, offset: isize) -> i32 {
//         unsafe { *(self.b.offset(offset) as *const i32) }
//     }
//     #[inline(always)]
//     fn set_i32ne(&mut self, offset: isize, value: i32) {
//         unsafe { *(self.b.offset(offset) as *mut i32) = value; }
//     }
//
//     ////////////////////////////////////////////////////////////////
//     // i64
//     ////////////////////////////////////////////////////////////////
//
//     #[inline(always)]
//     fn i64le(&self, offset: isize) -> i64 {
//         unsafe { i64::from_le(*(self.b.offset(offset) as *const i64)) }
//     }
//     #[inline(always)]
//     fn set_i64le(&mut self, offset: isize, value: i64) {
//         unsafe { *(self.b.offset(offset) as *mut i64) = value.to_le(); }
//     }
//     #[inline(always)]
//     fn i64be(&self, offset: isize) -> i64 {
//         unsafe { i64::from_be(*(self.b.offset(offset) as *const i64)) }
//     }
//     #[inline(always)]
//     fn set_i64be(&mut self, offset: isize, value: i64) {
//         unsafe { *(self.b.offset(offset) as *mut i64) = value.to_be(); }
//     }
//     #[inline(always)]
//     fn i64ne(&self, offset: isize) -> i64 {
//         unsafe { *(self.b.offset(offset) as *const i64) }
//     }
//     #[inline(always)]
//     fn set_i64ne(&mut self, offset: isize, value: i64) {
//         unsafe { *(self.b.offset(offset) as *mut i64) = value; }
//     }
//
//     ////////////////////////////////////////////////////////////////
//     // i128
//     ////////////////////////////////////////////////////////////////
//
//     #[inline(always)]
//     fn i128le(&self, offset: isize) -> i128 {
//         unsafe { i128::from_le(*(self.b.offset(offset) as *const i128)) }
//     }
//     #[inline(always)]
//     fn set_i128le(&mut self, offset: isize, value: i128) {
//         unsafe { *(self.b.offset(offset) as *mut i128) = value.to_le(); }
//     }
//     #[inline(always)]
//     fn i128be(&self, offset: isize) -> i128 {
//         unsafe { i128::from_be(*(self.b.offset(offset) as *const i128)) }
//     }
//     #[inline(always)]
//     fn set_i128be(&mut self, offset: isize, value: i128) {
//         unsafe { *(self.b.offset(offset) as *mut i128) = value.to_be(); }
//     }
//     #[inline(always)]
//     fn i128ne(&self, offset: isize) -> i128 {
//         unsafe { *(self.b.offset(offset) as *const i128) }
//     }
//     #[inline(always)]
//     fn set_i128ne(&mut self, offset: isize, value: i128) {
//         unsafe { *(self.b.offset(offset) as *mut i128) = value; }
//     }
//
//     ////////////////////////////////////////////////////////////////
//     // u16
//     ////////////////////////////////////////////////////////////////
//
//     #[inline(always)]
//     fn u16le(&self, offset: isize) -> u16 {
//         unsafe { u16::from_le(*(self.b.offset(offset) as *const u16)) }
//     }
//     #[inline(always)]
//     fn set_u16le(&mut self, offset: isize, value: u16) {
//         unsafe { *(self.b.offset(offset) as *mut u16) = value.to_le(); }
//     }
//     #[inline(always)]
//     fn u16be(&self, offset: isize) -> u16 {
//         unsafe { u16::from_be(*(self.b.offset(offset) as *const u16)) }
//     }
//     #[inline(always)]
//     fn set_u16be(&mut self, offset: isize, value: u16) {
//         unsafe { *(self.b.offset(offset) as *mut u16) = value.to_be(); }
//     }
//     #[inline(always)]
//     fn u16ne(&self, offset: isize) -> u16 {
//         unsafe { *(self.b.offset(offset) as *const u16) }
//     }
//     #[inline(always)]
//     fn set_u16ne(&mut self, offset: isize, value: u16) {
//         unsafe { *(self.b.offset(offset) as *mut u16) = value; }
//     }
//
//     ////////////////////////////////////////////////////////////////
//     // u32
//     ////////////////////////////////////////////////////////////////
//
//     #[inline(always)]
//     fn u32le(&self, offset: isize) -> u32 {
//         unsafe { u32::from_le(*(self.b.offset(offset) as *const u32)) }
//     }
//     #[inline(always)]
//     fn set_u32le(&mut self, offset: isize, value: u32) {
//         unsafe { *(self.b.offset(offset) as *mut u32) = value.to_le(); }
//     }
//     #[inline(always)]
//     fn u32be(&self, offset: isize) -> u32 {
//         unsafe { u32::from_be(*(self.b.offset(offset) as *const u32)) }
//     }
//     #[inline(always)]
//     fn set_u32be(&mut self, offset: isize, value: u32) {
//         unsafe { *(self.b.offset(offset) as *mut u32) = value.to_be(); }
//     }
//     #[inline(always)]
//     fn u32ne(&self, offset: isize) -> u32 {
//         unsafe { *(self.b.offset(offset) as *const u32) }
//     }
//     #[inline(always)]
//     fn set_u32ne(&mut self, offset: isize, value: u32) {
//         unsafe { *(self.b.offset(offset) as *mut u32) = value; }
//     }
//
//     ////////////////////////////////////////////////////////////////
//     // u64
//     ////////////////////////////////////////////////////////////////
//
//     #[inline(always)]
//     fn u64le(&self, offset: isize) -> u64 {
//         unsafe { u64::from_le(*(self.b.offset(offset) as *const u64)) }
//     }
//     #[inline(always)]
//     fn set_u64le(&mut self, offset: isize, value: u64) {
//         unsafe { *(self.b.offset(offset) as *mut u64) = value.to_le(); }
//     }
//     #[inline(always)]
//     fn u64be(&self, offset: isize) -> u64 {
//         unsafe { u64::from_be(*(self.b.offset(offset) as *const u64)) }
//     }
//     #[inline(always)]
//     fn set_u64be(&mut self, offset: isize, value: u64) {
//         unsafe { *(self.b.offset(offset) as *mut u64) = value.to_be(); }
//     }
//     #[inline(always)]
//     fn u64ne(&self, offset: isize) -> u64 {
//         unsafe { *(self.b.offset(offset) as *const u64) }
//     }
//     #[inline(always)]
//     fn set_u64ne(&mut self, offset: isize, value: u64) {
//         unsafe { *(self.b.offset(offset) as *mut u64) = value; }
//     }
//
//     ////////////////////////////////////////////////////////////////
//     // u128
//     ////////////////////////////////////////////////////////////////
//
//     #[inline(always)]
//     fn u128le(&self, offset: isize) -> u128 {
//         unsafe { u128::from_le(*(self.b.offset(offset) as *const u128)) }
//     }
//     #[inline(always)]
//     fn set_u128le(&mut self, offset: isize, value: u128) {
//         unsafe { *(self.b.offset(offset) as *mut u128) = value.to_le(); }
//     }
//     #[inline(always)]
//     fn u128be(&self, offset: isize) -> u128 {
//         unsafe { u128::from_be(*(self.b.offset(offset) as *const u128)) }
//     }
//     #[inline(always)]
//     fn set_u128be(&mut self, offset: isize, value: u128) {
//         unsafe { *(self.b.offset(offset) as *mut u128) = value.to_be(); }
//     }
//     #[inline(always)]
//     fn u128ne(&self, offset: isize) -> u128 {
//         unsafe { *(self.b.offset(offset) as *const u128) }
//     }
//     #[inline(always)]
//     fn set_u128ne(&mut self, offset: isize, value: u128) {
//         unsafe { *(self.b.offset(offset) as *mut u128) = value; }
//     }
//
//     ////////////////////////////////////////////////////////////////
//     // f32
//     ////////////////////////////////////////////////////////////////
//
//     #[inline(always)]
//     fn f32le(&self, offset: isize) -> f32 {
//         unsafe { f32::from_le_bytes(*(self.b.offset(offset) as *const [u8; 4])) }
//     }
//     #[inline(always)]
//     fn set_f32le(&mut self, offset: isize, value: f32) {
//         unsafe { *(self.b.offset(offset) as *mut [u8; 4]) = value.to_le_bytes(); }
//     }
//     #[inline(always)]
//     fn f32be(&self, offset: isize) -> f32 {
//         unsafe { f32::from_be_bytes(*(self.b.offset(offset) as *const [u8; 4])) }
//     }
//     #[inline(always)]
//     fn set_f32be(&mut self, offset: isize, value: f32) {
//         unsafe { *(self.b.offset(offset) as *mut [u8; 4]) = value.to_be_bytes(); }
//     }
//     #[inline(always)]
//     fn f32ne(&self, offset: isize) -> f32 {
//         unsafe { *(self.b.offset(offset) as *const f32) }
//     }
//     #[inline(always)]
//     fn set_f32ne(&mut self, offset: isize, value: f32) {
//         unsafe { *(self.b.offset(offset) as *mut f32) = value; }
//     }
//
//     ////////////////////////////////////////////////////////////////
//     // f64
//     ////////////////////////////////////////////////////////////////
//
//     #[inline(always)]
//     fn f64le(&self, offset: isize) -> f64 {
//         unsafe { f64::from_le_bytes(*(self.b.offset(offset) as *const [u8; 8])) }
//     }
//     #[inline(always)]
//     fn set_f64le(&mut self, offset: isize, value: f64) {
//         unsafe { *(self.b.offset(offset) as *mut [u8; 8]) = value.to_le_bytes(); }
//     }
//     #[inline(always)]
//     fn f64be(&self, offset: isize) -> f64 {
//         unsafe { f64::from_be_bytes(*(self.b.offset(offset) as *const [u8; 8])) }
//     }
//     #[inline(always)]
//     fn set_f64be(&mut self, offset: isize, value: f64) {
//         unsafe { *(self.b.offset(offset) as *mut [u8; 8]) = value.to_be_bytes(); }
//     }
//     #[inline(always)]
//     fn f64ne(&self, offset: isize) -> f64 {
//         unsafe { *(self.b.offset(offset) as *const f64) }
//     }
//     #[inline(always)]
//     fn set_f64ne(&mut self, offset: isize, value: f64) {
//         unsafe { *(self.b.offset(offset) as *mut f64) = value; }
//     }
//
//     fn str(&self, offset: isize, _: usize) -> &str {
//         unsafe {
//             let p = self.b.offset(offset);
//             let length = Self::Block::get_size_value_mut(p) as usize;
//             core::str::from_utf8_unchecked(
//                 &*core::ptr::slice_from_raw_parts(p.offset(Self::Block::SIZE_BYTES), length))
//         }
//     }
//
//     fn str_slice(&self, offset: isize, _: usize) -> &[u8] {
//         unsafe {
//             let p = self.b.offset(offset);
//             let length = Self::Block::get_size_value_mut(p) as usize;
//             &*core::ptr::slice_from_raw_parts(p.offset(Self::Block::SIZE_BYTES), length)
//         }
//     }
//
//     fn put_str(&mut self, offset: isize, capacity: usize, value: &str) {
//         unsafe {
//             let p = self.b.offset(offset);
//             let mut length = value.len();
//             if length > capacity - Self::Block::SIZE_BYTES_USIZE {
//                 length = capacity - Self::Block::SIZE_BYTES_USIZE;
//             }
//             Self::Block::put_size_value_usize(p, length);
//             if length > 0 {
//                 ptr::copy(value.as_ptr(), p.offset(Self::Block::SIZE_BYTES), length);
//             }
//         }
//     }
//
//     fn put_str_slice(&mut self, offset: isize, capacity: usize, value: &[u8]) {
//         unsafe {
//             let p = self.b.offset(offset);
//             let mut length = value.len();
//             if length > capacity - Self::Block::SIZE_BYTES_USIZE {
//                 length = capacity - Self::Block::SIZE_BYTES_USIZE;
//             }
//             Self::Block::put_size_value_usize(p, length);
//             if length > 0 {
//                 ptr::copy(value.as_ptr(), p.offset(Self::Block::SIZE_BYTES), length);
//             }
//         }
//     }
//
//     #[inline(always)]
//     fn slice(&self, offset: isize, capacity: usize) -> &[u8] {
//         unsafe {
//             &*core::ptr::slice_from_raw_parts(self.b.offset(offset), capacity)
//         }
//     }
//
//     #[inline(always)]
//     fn slice_mut(&mut self, offset: isize, capacity: usize) -> &mut [u8] {
//         unsafe {
//             &mut *core::ptr::slice_from_raw_parts_mut(self.b.offset(offset), capacity)
//         }
//     }
// }
//
// impl<M: Message> Slice for FixedUnsafe<M> {
//     type Block = M::Block;
//     type Size = M::Size;
//
//     #[inline(always)]
//     fn ptr(&self) -> *const u8 {
//         self.b
//     }
//
//     #[inline(always)]
//     fn ptr_mut(&mut self) -> *mut u8 {
//         self.b
//     }
//
//     ////////////////////////////////////////////////////////////////
//     // bool
//     ////////////////////////////////////////////////////////////////
//
//     #[inline(always)]
//     fn bool(&self, offset: isize) -> bool {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p < self.end { *p != 0 } else { false }
//         }
//     }
//     #[inline(always)]
//     fn set_bool(&mut self, offset: isize, value: bool) {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p < self.end { *p = if value { 1 } else { 0 } }
//         }
//     }
//
//     ////////////////////////////////////////////////////////////////
//     // i8
//     ////////////////////////////////////////////////////////////////
//
//     #[inline(always)]
//     fn i8(&self, offset: isize) -> i8 {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p < self.end { *p as i8 } else { 0 }
//         }
//     }
//     #[inline(always)]
//     fn set_i8(&mut self, offset: isize, value: i8) {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p < self.end { *p = value as u8 }
//         }
//     }
//
//     ////////////////////////////////////////////////////////////////
//     // u8
//     ////////////////////////////////////////////////////////////////
//
//     #[inline(always)]
//     fn u8(&self, offset: isize) -> u8 {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p < self.end { *p } else { 0 }
//         }
//     }
//     #[inline(always)]
//     fn set_u8(&mut self, offset: isize, value: u8) {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p < self.end { *p = value }
//         }
//     }
//
//
//     ////////////////////////////////////////////////////////////////
//     // i16
//     ////////////////////////////////////////////////////////////////
//
//     #[inline(always)]
//     fn i16le(&self, offset: isize) -> i16 {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(2) > self.end { 0 } else { i16::from_le(*(p as *const i16)) }
//         }
//     }
//     #[inline(always)]
//     fn set_i16le(&mut self, offset: isize, value: i16) {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(2) > self.end {
//                 return;
//             }
//             *(p as *mut i16) = value.to_le();
//         }
//     }
//     #[inline(always)]
//     fn i16be(&self, offset: isize) -> i16 {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(2) > self.end { 0 } else { i16::from_be(*(p as *const i16)) }
//         }
//     }
//     #[inline(always)]
//     fn set_i16be(&mut self, offset: isize, value: i16) {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(2) > self.end {
//                 return;
//             }
//             *(p as *mut i16) = value.to_be();
//         }
//     }
//     #[inline(always)]
//     fn i16ne(&self, offset: isize) -> i16 {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(2) > self.end { 0 } else { *(p as *const i16) }
//         }
//     }
//     #[inline(always)]
//     fn set_i16ne(&mut self, offset: isize, value: i16) {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(2) > self.end {
//                 return;
//             }
//             *(p as *mut i16) = value;
//         }
//     }
//
//     ////////////////////////////////////////////////////////////////
//     // i32
//     ////////////////////////////////////////////////////////////////
//
//     #[inline(always)]
//     fn i32le(&self, offset: isize) -> i32 {
//         unsafe {
//             if self.b.offset(offset + 4) > self.end { 0 } else { i32::from_le(*(self.b.offset(offset) as *const i32)) }
//         }
//     }
//     #[inline(always)]
//     fn set_i32le(&mut self, offset: isize, value: i32) {
//         unsafe {
//             if self.b.offset(offset + 4) > self.end {
//                 return;
//             }
//             *(self.b.offset(offset) as *mut i32) = value.to_le();
//         }
//     }
//     #[inline(always)]
//     fn i32be(&self, offset: isize) -> i32 {
//         unsafe {
//             if self.b.offset(offset + 4) > self.end { 0 } else { i32::from_be(*(self.b.offset(offset) as *const i32)) }
//         }
//     }
//     #[inline(always)]
//     fn set_i32be(&mut self, offset: isize, value: i32) {
//         unsafe {
//             if self.b.offset(offset + 4) > self.end {
//                 return;
//             }
//             *(self.b.offset(offset) as *mut i32) = value.to_be();
//         }
//     }
//     #[inline(always)]
//     fn i32ne(&self, offset: isize) -> i32 {
//         unsafe {
//             if self.b.offset(offset + 4) > self.end { 0 } else { *(self.b.offset(offset) as *const i32) }
//         }
//     }
//     #[inline(always)]
//     fn set_i32ne(&mut self, offset: isize, value: i32) {
//         unsafe {
//             if self.b.offset(offset + 4) > self.end {
//                 return;
//             }
//             *(self.b.offset(offset) as *mut i32) = value;
//         }
//     }
//
//
//     ////////////////////////////////////////////////////////////////
//     // i64
//     ////////////////////////////////////////////////////////////////
//
//     #[inline(always)]
//     fn i64le(&self, offset: isize) -> i64 {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(8) > self.end { 0 } else { i64::from_le(*(p as *const i64)) }
//         }
//     }
//     #[inline(always)]
//     fn set_i64le(&mut self, offset: isize, value: i64) {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(8) > self.end {
//                 return;
//             }
//             *(p as *mut i64) = value.to_le();
//         }
//     }
//     #[inline(always)]
//     fn i64be(&self, offset: isize) -> i64 {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(8) > self.end { 0 } else { i64::from_be(*(p as *const i64)) }
//         }
//     }
//     #[inline(always)]
//     fn set_i64be(&mut self, offset: isize, value: i64) {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(8) > self.end {
//                 return;
//             }
//             *(p as *mut i64) = value.to_be();
//         }
//     }
//     #[inline(always)]
//     fn i64ne(&self, offset: isize) -> i64 {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(8) > self.end { 0 } else { *(p as *const i64) }
//         }
//     }
//     #[inline(always)]
//     fn set_i64ne(&mut self, offset: isize, value: i64) {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(8) > self.end {
//                 return;
//             }
//             *(p as *mut i64) = value;
//         }
//     }
//
//     ////////////////////////////////////////////////////////////////
//     // i128
//     ////////////////////////////////////////////////////////////////
//
//     #[inline(always)]
//     fn i128le(&self, offset: isize) -> i128 {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(16) > self.end { 0 } else { i128::from_le(*(p as *const i128)) }
//         }
//     }
//     #[inline(always)]
//     fn set_i128le(&mut self, offset: isize, value: i128) {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(16) > self.end {
//                 return;
//             }
//             *(p as *mut i128) = value.to_le();
//         }
//     }
//     #[inline(always)]
//     fn i128be(&self, offset: isize) -> i128 {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(16) > self.end { 0 } else { i128::from_be(*(p as *const i128)) }
//         }
//     }
//     #[inline(always)]
//     fn set_i128be(&mut self, offset: isize, value: i128) {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(16) > self.end {
//                 return;
//             }
//             *(p as *mut i128) = value.to_be();
//         }
//     }
//     #[inline(always)]
//     fn i128ne(&self, offset: isize) -> i128 {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(16) > self.end { 0 } else { *(p as *const i128) }
//         }
//     }
//     #[inline(always)]
//     fn set_i128ne(&mut self, offset: isize, value: i128) {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(16) > self.end {
//                 return;
//             }
//             *(p as *mut i128) = value;
//         }
//     }
//
//
//     ////////////////////////////////////////////////////////////////
//     // u16
//     ////////////////////////////////////////////////////////////////
//
//     #[inline(always)]
//     fn u16le(&self, offset: isize) -> u16 {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(2) > self.end { 0 } else { u16::from_le(*(p as *const u16)) }
//         }
//     }
//     #[inline(always)]
//     fn set_u16le(&mut self, offset: isize, value: u16) {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(2) > self.end {
//                 return;
//             }
//             *(p as *mut u16) = value.to_le();
//         }
//     }
//     #[inline(always)]
//     fn u16be(&self, offset: isize) -> u16 {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(2) > self.end { 0 } else { u16::from_be(*(p as *const u16)) }
//         }
//     }
//     #[inline(always)]
//     fn set_u16be(&mut self, offset: isize, value: u16) {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(2) > self.end {
//                 return;
//             }
//             *(p as *mut u16) = value.to_be();
//         }
//     }
//     #[inline(always)]
//     fn u16ne(&self, offset: isize) -> u16 {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(2) > self.end { 0 } else { *(p as *const u16) }
//         }
//     }
//     #[inline(always)]
//     fn set_u16ne(&mut self, offset: isize, value: u16) {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(2) > self.end {
//                 return;
//             }
//             *(p as *mut u16) = value;
//         }
//     }
//
//     ////////////////////////////////////////////////////////////////
//     // u32
//     ////////////////////////////////////////////////////////////////
//
//     #[inline(always)]
//     fn u32le(&self, offset: isize) -> u32 {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(4) > self.end { 0 } else { u32::from_le(*(p as *const u32)) }
//         }
//     }
//     #[inline(always)]
//     fn set_u32le(&mut self, offset: isize, value: u32) {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(4) > self.end {
//                 return;
//             }
//             *(p as *mut u32) = value.to_le();
//         }
//     }
//     #[inline(always)]
//     fn u32be(&self, offset: isize) -> u32 {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(4) > self.end { 0 } else { u32::from_be(*(p as *const u32)) }
//         }
//     }
//     #[inline(always)]
//     fn set_u32be(&mut self, offset: isize, value: u32) {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(4) > self.end {
//                 return;
//             }
//             *(p as *mut u32) = value.to_be();
//         }
//     }
//     #[inline(always)]
//     fn u32ne(&self, offset: isize) -> u32 {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(4) > self.end { 0 } else { *(p as *const u32) }
//         }
//     }
//     #[inline(always)]
//     fn set_u32ne(&mut self, offset: isize, value: u32) {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(4) > self.end {
//                 return;
//             }
//             *(p as *mut u32) = value;
//         }
//     }
//
//
//     ////////////////////////////////////////////////////////////////
//     // u64
//     ////////////////////////////////////////////////////////////////
//
//     #[inline(always)]
//     fn u64le(&self, offset: isize) -> u64 {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(8) > self.end { 0 } else { u64::from_le(*(p as *const u64)) }
//         }
//     }
//     #[inline(always)]
//     fn set_u64le(&mut self, offset: isize, value: u64) {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(8) > self.end {
//                 return;
//             }
//             *(p as *mut u64) = value.to_le();
//         }
//     }
//     #[inline(always)]
//     fn u64be(&self, offset: isize) -> u64 {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(8) > self.end { 0 } else { u64::from_be(*(p as *const u64)) }
//         }
//     }
//     #[inline(always)]
//     fn set_u64be(&mut self, offset: isize, value: u64) {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(8) > self.end {
//                 return;
//             }
//             *(p as *mut u64) = value.to_be();
//         }
//     }
//     #[inline(always)]
//     fn u64ne(&self, offset: isize) -> u64 {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(8) > self.end { 0 } else { *(p as *const u64) }
//         }
//     }
//     #[inline(always)]
//     fn set_u64ne(&mut self, offset: isize, value: u64) {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(8) > self.end {
//                 return;
//             }
//             *(p as *mut u64) = value;
//         }
//     }
//
//     ////////////////////////////////////////////////////////////////
//     // u128
//     ////////////////////////////////////////////////////////////////
//
//     #[inline(always)]
//     fn u128le(&self, offset: isize) -> u128 {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(16) > self.end { 0 } else { u128::from_le(*(p as *const u128)) }
//         }
//     }
//     #[inline(always)]
//     fn set_u128le(&mut self, offset: isize, value: u128) {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(16) > self.end {
//                 return;
//             }
//             *(p as *mut u128) = value.to_le();
//         }
//     }
//     #[inline(always)]
//     fn u128be(&self, offset: isize) -> u128 {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(16) > self.end { 0 } else { u128::from_be(*(p as *const u128)) }
//         }
//     }
//     #[inline(always)]
//     fn set_u128be(&mut self, offset: isize, value: u128) {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(16) > self.end {
//                 return;
//             }
//             *(p as *mut u128) = value.to_be();
//         }
//     }
//     #[inline(always)]
//     fn u128ne(&self, offset: isize) -> u128 {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(16) > self.end { 0 } else { *(p as *const u128) }
//         }
//     }
//     #[inline(always)]
//     fn set_u128ne(&mut self, offset: isize, value: u128) {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(16) > self.end {
//                 return;
//             }
//             *(p as *mut u128) = value;
//         }
//     }
//
//
//     ////////////////////////////////////////////////////////////////
//     // f32
//     ////////////////////////////////////////////////////////////////
//
//     #[inline(always)]
//     fn f32le(&self, offset: isize) -> f32 {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(4) > self.end { 0f32 } else { f32::from_le_bytes(*(p as *const f32 as *const [u8; 4])) }
//         }
//     }
//     #[inline(always)]
//     fn set_f32le(&mut self, offset: isize, value: f32) {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(4) > self.end {
//                 return;
//             }
//             *(p as *mut [u8; 4]) = value.to_le_bytes();
//         }
//     }
//     #[inline(always)]
//     fn f32be(&self, offset: isize) -> f32 {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(4) > self.end { 0f32 } else { f32::from_be_bytes(*(p as *const f32 as *const [u8; 4])) }
//         }
//     }
//     #[inline(always)]
//     fn set_f32be(&mut self, offset: isize, value: f32) {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(4) > self.end {
//                 return;
//             }
//             *(p as *mut [u8; 4]) = value.to_be_bytes();
//         }
//     }
//     #[inline(always)]
//     fn f32ne(&self, offset: isize) -> f32 {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(4) > self.end { 0f32 } else { *(p as *const f32) }
//         }
//     }
//     #[inline(always)]
//     fn set_f32ne(&mut self, offset: isize, value: f32) {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(4) > self.end {
//                 return;
//             }
//             *(p as *mut f32) = value;
//         }
//     }
//
//     ////////////////////////////////////////////////////////////////
//     // f64
//     ////////////////////////////////////////////////////////////////
//
//     #[inline(always)]
//     fn f64le(&self, offset: isize) -> f64 {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(8) > self.end { 0f64 } else { f64::from_le_bytes(*(p as *const f64 as *const [u8; 8])) }
//         }
//     }
//     #[inline(always)]
//     fn set_f64le(&mut self, offset: isize, value: f64) {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(8) > self.end {
//                 return;
//             }
//             *(p as *mut [u8; 8]) = value.to_le_bytes();
//         }
//     }
//     #[inline(always)]
//     fn f64be(&self, offset: isize) -> f64 {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(8) > self.end { 0f64 } else { f64::from_be_bytes(*(p as *const f64 as *const [u8; 8])) }
//         }
//     }
//     #[inline(always)]
//     fn set_f64be(&mut self, offset: isize, value: f64) {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(8) > self.end {
//                 return;
//             }
//             *(p as *mut [u8; 8]) = value.to_be_bytes();
//         }
//     }
//     #[inline(always)]
//     fn f64ne(&self, offset: isize) -> f64 {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(8) > self.end { 0f64 } else { *(p as *const f64) }
//         }
//     }
//     #[inline(always)]
//     fn set_f64ne(&mut self, offset: isize, value: f64) {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(8) > self.end {
//                 return;
//             }
//             *(p as *mut f64) = value;
//         }
//     }
//
//     fn str(&self, offset: isize, capacity: usize) -> &str {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(capacity as isize) > self.end {
//                 return "";
//             }
//             let length = Self::Block::get_size_value_mut(p) as usize;
//             let p = p.offset(Self::Block::SIZE_BYTES);
//             if p.offset(length as isize) > self.end {
//                 return "";
//             }
//             core::str::from_utf8_unchecked(
//                 &*core::ptr::slice_from_raw_parts(p, length))
//         }
//     }
//
//     fn str_slice(&self, offset: isize, capacity: usize) -> &[u8] {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(capacity as isize) > self.end {
//                 return &*core::ptr::slice_from_raw_parts(ptr::null::<u8>(), 0);
//             }
//             let length = Self::Block::get_size_value_mut(p) as usize;
//             if p.offset(length as isize) > self.end {
//                 return &*core::ptr::slice_from_raw_parts(ptr::null::<u8>(), 0);
//             }
//             &*core::ptr::slice_from_raw_parts(p, length)
//         }
//     }
//
//     fn put_str(&mut self, offset: isize, capacity: usize, value: &str) {
//         unsafe {
//             if self.b.offset(offset + capacity as isize) > self.end {
//                 return;
//             }
//             let p = self.b.offset(offset);
//
//             let mut length = value.len();
//             if length > capacity - Self::Block::SIZE_BYTES_USIZE {
//                 length = capacity - Self::Block::SIZE_BYTES_USIZE;
//             }
//             Self::Block::put_size_value_usize(p, length);
//             if length > 0 {
//                 ptr::copy(value.as_ptr(), p.offset(Self::Block::SIZE_BYTES), length);
//             }
//         }
//     }
//
//     fn put_str_slice(&mut self, offset: isize, capacity: usize, value: &[u8]) {
//         unsafe {
//             if self.b.offset(offset + capacity as isize) > self.end {
//                 return;
//             }
//             let p = self.b.offset(offset);
//
//             let mut length = value.len();
//             if length > capacity - Self::Block::SIZE_BYTES_USIZE {
//                 length = capacity - Self::Block::SIZE_BYTES_USIZE;
//             }
//             Self::Block::put_size_value_usize(p, length);
//             if length > 0 {
//                 ptr::copy(value.as_ptr(), p.offset(Self::Block::SIZE_BYTES), length);
//             }
//         }
//     }
//
//     #[inline(always)]
//     fn slice(&self, offset: isize, capacity: usize) -> &[u8] {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(capacity as isize) > self.end {
//                 return &*core::ptr::slice_from_raw_parts(ptr::null::<u8>(), 0);
//             }
//             &*core::ptr::slice_from_raw_parts(p.offset(offset), capacity)
//         }
//     }
//
//     #[inline(always)]
//     fn slice_mut(&mut self, offset: isize, capacity: usize) -> &mut [u8] {
//         unsafe {
//             let p = self.b.offset(offset);
//             if p.offset(capacity as isize) > self.end {
//                 return &mut *core::ptr::slice_from_raw_parts_mut(ptr::null_mut(), 0);
//             }
//             &mut *core::ptr::slice_from_raw_parts_mut(p.offset(offset) as *mut u8, capacity)
//         }
//     }
// }
//
//
// pub struct FlexSafe<M: Message> {
//     m: *const M,
//     s: FixedSafe<M>,
//     // _block: PhantomData<B>,
// }
//
// impl<M: Message> FlexSafe<M> {
//     pub fn new(m: &M, p: *mut u8) -> Self {
//         Self { m: &*m, s: FixedSafe { b: p, _block: PhantomData::<M> } }
//     }
//     pub fn message(&self) -> &M {
//         unsafe { &*self.m }
//     }
// }
//
// pub struct FlexUnsafe<M: Message> {
//     m: *const M,
//     s: FixedUnsafe<M>,
//     // _block: PhantomData<B>,
// }
//
// impl<M: Message> Message for FlexSafe<M> {
//     type Block = M::Block;
//     type Size = M::Size;
//     type Record = M::Record;
//
//     #[inline(always)]
//     fn get(&self) -> &Self {
//         self
//     }
//
//     #[inline(always)]
//     fn base(&self) -> *mut u8 {
//         unsafe { (&*(self.m)).base() }
//     }
//
//     #[inline(always)]
//     fn tail(&self) -> *mut u8 {
//         unsafe { (&*(self.m)).tail() }
//     }
// }
//
// impl<M: Message> Slice for FlexSafe<M> {
//     type Block = M::Block;
//     type Size = M::Size;
//
//     #[inline(always)]
//     fn ptr(&self) -> *const u8 {
//         self.s.ptr()
//     }
//
//     #[inline(always)]
//     fn ptr_mut(&mut self) -> *mut u8 {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn bool(&self, offset: isize) -> bool {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn set_bool(&mut self, offset: isize, value: bool) {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn i8(&self, offset: isize) -> i8 {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn set_i8(&mut self, offset: isize, value: i8) {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn u8(&self, offset: isize) -> u8 {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn set_u8(&mut self, offset: isize, value: u8) {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn i16le(&self, offset: isize) -> i16 {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn set_i16le(&mut self, offset: isize, value: i16) {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn i16be(&self, offset: isize) -> i16 {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn set_i16be(&mut self, offset: isize, value: i16) {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn i16ne(&self, offset: isize) -> i16 {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn set_i16ne(&mut self, offset: isize, value: i16) {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn i32le(&self, offset: isize) -> i32 {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn set_i32le(&mut self, offset: isize, value: i32) {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn i32be(&self, offset: isize) -> i32 {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn set_i32be(&mut self, offset: isize, value: i32) {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn i32ne(&self, offset: isize) -> i32 {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn set_i32ne(&mut self, offset: isize, value: i32) {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn i64le(&self, offset: isize) -> i64 {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn set_i64le(&mut self, offset: isize, value: i64) {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn i64be(&self, offset: isize) -> i64 {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn set_i64be(&mut self, offset: isize, value: i64) {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn i64ne(&self, offset: isize) -> i64 {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn set_i64ne(&mut self, offset: isize, value: i64) {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn i128le(&self, offset: isize) -> i128 {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn set_i128le(&mut self, offset: isize, value: i128) {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn i128be(&self, offset: isize) -> i128 {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn set_i128be(&mut self, offset: isize, value: i128) {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn i128ne(&self, offset: isize) -> i128 {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn set_i128ne(&mut self, offset: isize, value: i128) {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn u16le(&self, offset: isize) -> u16 {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn set_u16le(&mut self, offset: isize, value: u16) {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn u16be(&self, offset: isize) -> u16 {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn set_u16be(&mut self, offset: isize, value: u16) {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn u16ne(&self, offset: isize) -> u16 {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn set_u16ne(&mut self, offset: isize, value: u16) {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn u32le(&self, offset: isize) -> u32 {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn set_u32le(&mut self, offset: isize, value: u32) {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn u32be(&self, offset: isize) -> u32 {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn set_u32be(&mut self, offset: isize, value: u32) {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn u32ne(&self, offset: isize) -> u32 {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn set_u32ne(&mut self, offset: isize, value: u32) {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn u64le(&self, offset: isize) -> u64 {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn set_u64le(&mut self, offset: isize, value: u64) {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn u64be(&self, offset: isize) -> u64 {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn set_u64be(&mut self, offset: isize, value: u64) {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn u64ne(&self, offset: isize) -> u64 {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn set_u64ne(&mut self, offset: isize, value: u64) {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn u128le(&self, offset: isize) -> u128 {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn set_u128le(&mut self, offset: isize, value: u128) {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn u128be(&self, offset: isize) -> u128 {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn set_u128be(&mut self, offset: isize, value: u128) {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn u128ne(&self, offset: isize) -> u128 {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn set_u128ne(&mut self, offset: isize, value: u128) {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn f32le(&self, offset: isize) -> f32 {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn set_f32le(&mut self, offset: isize, value: f32) {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn f32be(&self, offset: isize) -> f32 {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn set_f32be(&mut self, offset: isize, value: f32) {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn f32ne(&self, offset: isize) -> f32 {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn set_f32ne(&mut self, offset: isize, value: f32) {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn f64le(&self, offset: isize) -> f64 {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn set_f64le(&mut self, offset: isize, value: f64) {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn f64be(&self, offset: isize) -> f64 {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn set_f64be(&mut self, offset: isize, value: f64) {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn f64ne(&self, offset: isize) -> f64 {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn set_f64ne(&mut self, offset: isize, value: f64) {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn str(&self, offset: isize, capacity: usize) -> &str {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn str_slice(&self, offset: isize, capacity: usize) -> &[u8] {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn put_str(&mut self, offset: isize, capacity: usize, value: &str) {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn put_str_slice(&mut self, offset: isize, capacity: usize, value: &[u8]) {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn slice(&self, offset: isize, capacity: usize) -> &[u8] {
//         todo!()
//     }
//
//     #[inline(always)]
//     fn slice_mut(&mut self, offset: isize, capacity: usize) -> &mut [u8] {
//         todo!()
//     }
// }
//
// pub struct Builder<B: Block> {
//     root: *mut *mut u8,
//     base: isize,
//     _p: PhantomData<B>,
// }
//
// pub type Builder16 = Builder<Block16>;
// pub type Builder32 = Builder<Block32>;
//
// impl<B: Block> Builder<B> {
//     #[inline(always)]
//     pub fn root(&self) -> *mut *mut u8 {
//         self.root
//     }
// }
//
// pub struct HeapBuilder<B: Block> {
//     root: *mut *mut u8,
//     base: isize,
//     _p: PhantomData<B>,
// }
//
// impl<B: Block> Slice for Builder<B> {
//     type Block = B;
//     type Size = B::Size;
//
//     #[inline(always)]
//     fn ptr(&self) -> *const u8 {
//         unsafe { (*self.root).offset(self.base) }
//     }
//
//     #[inline(always)]
//     fn ptr_mut(&mut self) -> *mut u8 {
//         unsafe { (*self.root).offset(self.base) }
//     }
//
//     ////////////////////////////////////////////////////////////////
//     // bool
//     ////////////////////////////////////////////////////////////////
//
//     #[inline(always)]
//     fn bool(&self, offset: isize) -> bool {
//         unsafe { *(self.ptr().offset(offset)) != 0 }
//     }
//     #[inline(always)]
//     fn set_bool(&mut self, offset: isize, value: bool) {
//         unsafe { *(self.ptr_mut().offset(offset)) = if value { 1 } else { 0 }; }
//     }
//
//     ////////////////////////////////////////////////////////////////
//     // i8
//     ////////////////////////////////////////////////////////////////
//     #[inline(always)]
//     fn i8(&self, offset: isize) -> i8 {
//         unsafe { *(self.ptr().offset(offset) as *const i8) }
//     }
//     #[inline(always)]
//     fn set_i8(&mut self, offset: isize, value: i8) {
//         unsafe { *(self.ptr_mut().offset(offset) as *mut i8) = value; }
//     }
//
//     ////////////////////////////////////////////////////////////////
//     // u8
//     ////////////////////////////////////////////////////////////////
//
//     #[inline(always)]
//     fn u8(&self, offset: isize) -> u8 {
//         unsafe { *(self.ptr().offset(offset)) }
//     }
//     #[inline(always)]
//     fn set_u8(&mut self, offset: isize, value: u8) {
//         unsafe { *(self.ptr_mut().offset(offset)) = value; }
//     }
//
//     ////////////////////////////////////////////////////////////////
//     // i16
//     ////////////////////////////////////////////////////////////////
//
//     #[inline(always)]
//     fn i16le(&self, offset: isize) -> i16 {
//         unsafe { i16::from_le(*(self.ptr().offset(offset) as *const i16)) }
//     }
//     #[inline(always)]
//     fn set_i16le(&mut self, offset: isize, value: i16) {
//         unsafe { *(self.ptr_mut().offset(offset) as *mut i16) = value.to_le(); }
//     }
//     #[inline(always)]
//     fn i16be(&self, offset: isize) -> i16 {
//         unsafe { i16::from_be(*(self.ptr().offset(offset) as *const i16)) }
//     }
//     #[inline(always)]
//     fn set_i16be(&mut self, offset: isize, value: i16) {
//         unsafe { *(self.ptr_mut().offset(offset) as *mut i16) = value.to_be(); }
//     }
//     #[inline(always)]
//     fn i16ne(&self, offset: isize) -> i16 {
//         unsafe { *(self.ptr().offset(offset) as *const i16) }
//     }
//     #[inline(always)]
//     fn set_i16ne(&mut self, offset: isize, value: i16) {
//         unsafe { *(self.ptr_mut().offset(offset) as *mut i16) = value; }
//     }
//
//     ////////////////////////////////////////////////////////////////
//     // i32
//     ////////////////////////////////////////////////////////////////
//
//     #[inline(always)]
//     fn i32le(&self, offset: isize) -> i32 {
//         unsafe { i32::from_le(*(self.ptr().offset(offset) as *const i32)) }
//     }
//     #[inline(always)]
//     fn set_i32le(&mut self, offset: isize, value: i32) {
//         unsafe { *(self.ptr_mut().offset(offset) as *mut i32) = value.to_le(); }
//     }
//     #[inline(always)]
//     fn i32be(&self, offset: isize) -> i32 {
//         unsafe { i32::from_be(*(self.ptr().offset(offset) as *const i32)) }
//     }
//     #[inline(always)]
//     fn set_i32be(&mut self, offset: isize, value: i32) {
//         unsafe { *(self.ptr_mut().offset(offset) as *mut i32) = value.to_be(); }
//     }
//     #[inline(always)]
//     fn i32ne(&self, offset: isize) -> i32 {
//         unsafe { *(self.ptr().offset(offset) as *const i32) }
//     }
//     #[inline(always)]
//     fn set_i32ne(&mut self, offset: isize, value: i32) {
//         unsafe { *(self.ptr_mut().offset(offset) as *mut i32) = value; }
//     }
//
//     ////////////////////////////////////////////////////////////////
//     // i64
//     ////////////////////////////////////////////////////////////////
//
//     #[inline(always)]
//     fn i64le(&self, offset: isize) -> i64 {
//         unsafe { i64::from_le(*(self.ptr().offset(offset) as *const i64)) }
//     }
//     #[inline(always)]
//     fn set_i64le(&mut self, offset: isize, value: i64) {
//         unsafe { *(self.ptr_mut().offset(offset) as *mut i64) = value.to_le(); }
//     }
//     #[inline(always)]
//     fn i64be(&self, offset: isize) -> i64 {
//         unsafe { i64::from_be(*(self.ptr().offset(offset) as *const i64)) }
//     }
//     #[inline(always)]
//     fn set_i64be(&mut self, offset: isize, value: i64) {
//         unsafe { *(self.ptr_mut().offset(offset) as *mut i64) = value.to_be(); }
//     }
//     #[inline(always)]
//     fn i64ne(&self, offset: isize) -> i64 {
//         unsafe { *(self.ptr().offset(offset) as *const i64) }
//     }
//     #[inline(always)]
//     fn set_i64ne(&mut self, offset: isize, value: i64) {
//         unsafe { *(self.ptr_mut().offset(offset) as *mut i64) = value; }
//     }
//
//     ////////////////////////////////////////////////////////////////
//     // i128
//     ////////////////////////////////////////////////////////////////
//
//     #[inline(always)]
//     fn i128le(&self, offset: isize) -> i128 {
//         unsafe { i128::from_le(*(self.ptr().offset(offset) as *const i128)) }
//     }
//     #[inline(always)]
//     fn set_i128le(&mut self, offset: isize, value: i128) {
//         unsafe { *(self.ptr_mut().offset(offset) as *mut i128) = value.to_le(); }
//     }
//     #[inline(always)]
//     fn i128be(&self, offset: isize) -> i128 {
//         unsafe { i128::from_be(*(self.ptr().offset(offset) as *const i128)) }
//     }
//     #[inline(always)]
//     fn set_i128be(&mut self, offset: isize, value: i128) {
//         unsafe { *(self.ptr_mut().offset(offset) as *mut i128) = value.to_be(); }
//     }
//     #[inline(always)]
//     fn i128ne(&self, offset: isize) -> i128 {
//         unsafe { *(self.ptr().offset(offset) as *const i128) }
//     }
//     #[inline(always)]
//     fn set_i128ne(&mut self, offset: isize, value: i128) {
//         unsafe { *(self.ptr_mut().offset(offset) as *mut i128) = value; }
//     }
//
//     ////////////////////////////////////////////////////////////////
//     // u16
//     ////////////////////////////////////////////////////////////////
//
//     #[inline(always)]
//     fn u16le(&self, offset: isize) -> u16 {
//         unsafe { u16::from_le(*(self.ptr().offset(offset) as *const u16)) }
//     }
//     #[inline(always)]
//     fn set_u16le(&mut self, offset: isize, value: u16) {
//         unsafe { *(self.ptr_mut().offset(offset) as *mut u16) = value.to_le(); }
//     }
//     #[inline(always)]
//     fn u16be(&self, offset: isize) -> u16 {
//         unsafe { u16::from_be(*(self.ptr().offset(offset) as *const u16)) }
//     }
//     #[inline(always)]
//     fn set_u16be(&mut self, offset: isize, value: u16) {
//         unsafe { *(self.ptr_mut().offset(offset) as *mut u16) = value.to_be(); }
//     }
//     #[inline(always)]
//     fn u16ne(&self, offset: isize) -> u16 {
//         unsafe { *(self.ptr().offset(offset) as *const u16) }
//     }
//     #[inline(always)]
//     fn set_u16ne(&mut self, offset: isize, value: u16) {
//         unsafe { *(self.ptr_mut().offset(offset) as *mut u16) = value; }
//     }
//
//     ////////////////////////////////////////////////////////////////
//     // u32
//     ////////////////////////////////////////////////////////////////
//
//     #[inline(always)]
//     fn u32le(&self, offset: isize) -> u32 {
//         unsafe { u32::from_le(*(self.ptr().offset(offset) as *const u32)) }
//     }
//     #[inline(always)]
//     fn set_u32le(&mut self, offset: isize, value: u32) {
//         unsafe { *(self.ptr_mut().offset(offset) as *mut u32) = value.to_le(); }
//     }
//     #[inline(always)]
//     fn u32be(&self, offset: isize) -> u32 {
//         unsafe { u32::from_be(*(self.ptr().offset(offset) as *const u32)) }
//     }
//     #[inline(always)]
//     fn set_u32be(&mut self, offset: isize, value: u32) {
//         unsafe { *(self.ptr_mut().offset(offset) as *mut u32) = value.to_be(); }
//     }
//     #[inline(always)]
//     fn u32ne(&self, offset: isize) -> u32 {
//         unsafe { *(self.ptr().offset(offset) as *const u32) }
//     }
//     #[inline(always)]
//     fn set_u32ne(&mut self, offset: isize, value: u32) {
//         unsafe { *(self.ptr_mut().offset(offset) as *mut u32) = value; }
//     }
//
//     ////////////////////////////////////////////////////////////////
//     // u64
//     ////////////////////////////////////////////////////////////////
//
//     #[inline(always)]
//     fn u64le(&self, offset: isize) -> u64 {
//         unsafe { u64::from_le(*(self.ptr().offset(offset) as *const u64)) }
//     }
//     #[inline(always)]
//     fn set_u64le(&mut self, offset: isize, value: u64) {
//         unsafe { *(self.ptr_mut().offset(offset) as *mut u64) = value.to_le(); }
//     }
//     #[inline(always)]
//     fn u64be(&self, offset: isize) -> u64 {
//         unsafe { u64::from_be(*(self.ptr().offset(offset) as *const u64)) }
//     }
//     #[inline(always)]
//     fn set_u64be(&mut self, offset: isize, value: u64) {
//         unsafe { *(self.ptr_mut().offset(offset) as *mut u64) = value.to_be(); }
//     }
//     #[inline(always)]
//     fn u64ne(&self, offset: isize) -> u64 {
//         unsafe { *(self.ptr().offset(offset) as *const u64) }
//     }
//     #[inline(always)]
//     fn set_u64ne(&mut self, offset: isize, value: u64) {
//         unsafe { *(self.ptr_mut().offset(offset) as *mut u64) = value; }
//     }
//
//     ////////////////////////////////////////////////////////////////
//     // u128
//     ////////////////////////////////////////////////////////////////
//
//     #[inline(always)]
//     fn u128le(&self, offset: isize) -> u128 {
//         unsafe { u128::from_le(*(self.ptr().offset(offset) as *const u128)) }
//     }
//     #[inline(always)]
//     fn set_u128le(&mut self, offset: isize, value: u128) {
//         unsafe { *(self.ptr_mut().offset(offset) as *mut u128) = value.to_le(); }
//     }
//     #[inline(always)]
//     fn u128be(&self, offset: isize) -> u128 {
//         unsafe { u128::from_be(*(self.ptr().offset(offset) as *const u128)) }
//     }
//     #[inline(always)]
//     fn set_u128be(&mut self, offset: isize, value: u128) {
//         unsafe { *(self.ptr_mut().offset(offset) as *mut u128) = value.to_be(); }
//     }
//     #[inline(always)]
//     fn u128ne(&self, offset: isize) -> u128 {
//         unsafe { *(self.ptr().offset(offset) as *const u128) }
//     }
//     #[inline(always)]
//     fn set_u128ne(&mut self, offset: isize, value: u128) {
//         unsafe { *(self.ptr_mut().offset(offset) as *mut u128) = value; }
//     }
//
//     ////////////////////////////////////////////////////////////////
//     // f32
//     ////////////////////////////////////////////////////////////////
//
//     #[inline(always)]
//     fn f32le(&self, offset: isize) -> f32 {
//         unsafe { f32::from_le_bytes(*(self.ptr().offset(offset) as *const [u8; 4])) }
//     }
//     #[inline(always)]
//     fn set_f32le(&mut self, offset: isize, value: f32) {
//         unsafe { *(self.ptr_mut().offset(offset) as *mut [u8; 4]) = value.to_le_bytes(); }
//     }
//     #[inline(always)]
//     fn f32be(&self, offset: isize) -> f32 {
//         unsafe { f32::from_be_bytes(*(self.ptr().offset(offset) as *const [u8; 4])) }
//     }
//     #[inline(always)]
//     fn set_f32be(&mut self, offset: isize, value: f32) {
//         unsafe { *(self.ptr_mut().offset(offset) as *mut [u8; 4]) = value.to_be_bytes(); }
//     }
//     #[inline(always)]
//     fn f32ne(&self, offset: isize) -> f32 {
//         unsafe { *(self.ptr().offset(offset) as *const f32) }
//     }
//     #[inline(always)]
//     fn set_f32ne(&mut self, offset: isize, value: f32) {
//         unsafe { *(self.ptr_mut().offset(offset) as *mut f32) = value; }
//     }
//
//     ////////////////////////////////////////////////////////////////
//     // f64
//     ////////////////////////////////////////////////////////////////
//
//     #[inline(always)]
//     fn f64le(&self, offset: isize) -> f64 {
//         unsafe { f64::from_le_bytes(*(self.ptr().offset(offset) as *const [u8; 8])) }
//     }
//     #[inline(always)]
//     fn set_f64le(&mut self, offset: isize, value: f64) {
//         unsafe { *(self.ptr_mut().offset(offset) as *mut [u8; 8]) = value.to_le_bytes(); }
//     }
//     #[inline(always)]
//     fn f64be(&self, offset: isize) -> f64 {
//         unsafe { f64::from_be_bytes(*(self.ptr().offset(offset) as *const [u8; 8])) }
//     }
//     #[inline(always)]
//     fn set_f64be(&mut self, offset: isize, value: f64) {
//         unsafe { *(self.ptr_mut().offset(offset) as *mut [u8; 8]) = value.to_be_bytes(); }
//     }
//     #[inline(always)]
//     fn f64ne(&self, offset: isize) -> f64 {
//         unsafe { *(self.ptr().offset(offset) as *const f64) }
//     }
//     #[inline(always)]
//     fn set_f64ne(&mut self, offset: isize, value: f64) {
//         unsafe { *(self.ptr_mut().offset(offset) as *mut f64) = value; }
//     }
//
//     fn str(&self, offset: isize, _: usize) -> &str {
//         unsafe {
//             let p = self.ptr().offset(offset);
//             let length = Self::Block::get_size_value(p) as usize;
//             core::str::from_utf8_unchecked(
//                 &*core::ptr::slice_from_raw_parts(p.offset(Self::Block::SIZE_BYTES), length))
//         }
//     }
//
//     fn str_slice(&self, offset: isize, _: usize) -> &[u8] {
//         unsafe {
//             let p = self.ptr().offset(offset);
//             let length = Self::Block::get_size_value(p) as usize;
//             &*core::ptr::slice_from_raw_parts(p.offset(Self::Block::SIZE_BYTES), length)
//         }
//     }
//
//     fn put_str(&mut self, offset: isize, capacity: usize, value: &str) {
//         unsafe {
//             let p = self.ptr_mut().offset(offset);
//             let mut length = value.len();
//             if length > capacity - Self::Block::SIZE_BYTES_USIZE {
//                 length = capacity - Self::Block::SIZE_BYTES_USIZE;
//             }
//             Self::Block::put_size_value_usize(p, length);
//             if length > 0 {
//                 ptr::copy(value.as_ptr(), p.offset(Self::Block::SIZE_BYTES), length);
//             }
//         }
//     }
//
//     fn put_str_slice(&mut self, offset: isize, capacity: usize, value: &[u8]) {
//         unsafe {
//             let p = self.ptr_mut().offset(offset);
//             let mut length = value.len();
//             if length > capacity - Self::Block::SIZE_BYTES_USIZE {
//                 length = capacity - Self::Block::SIZE_BYTES_USIZE;
//             }
//             Self::Block::put_size_value_usize(p, length);
//             if length > 0 {
//                 ptr::copy(value.as_ptr(), p.offset(Self::Block::SIZE_BYTES), length);
//             }
//         }
//     }
//
//     #[inline(always)]
//     fn slice(&self, offset: isize, capacity: usize) -> &[u8] {
//         unsafe {
//             &*core::ptr::slice_from_raw_parts(self.ptr().offset(offset), capacity)
//         }
//     }
//
//     #[inline(always)]
//     fn slice_mut(&mut self, offset: isize, capacity: usize) -> &mut [u8] {
//         unsafe {
//             &mut *core::ptr::slice_from_raw_parts_mut(self.ptr_mut().offset(offset), capacity)
//         }
//     }
// }
//
