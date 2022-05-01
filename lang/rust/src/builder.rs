/////////////////////////////////////////////////////////////////////////////
// BaseBuilder16Ref
/////////////////////////////////////////////////////////////////////////////

use std::{mem, ptr};
use std::borrow::BorrowMut;
use std::cell::UnsafeCell;
use std::error::Error;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ops::{Deref, DerefMut};
use std::pin::Pin;

use crate::{data, message};
use crate::alloc::{Allocator, Doubled, Global, Grow, TruncResult};
use crate::block::Block;
use crate::header::{Fixed16, Header, Header16};
use crate::message::Message;

pub trait Builder {
    type Header: Header;
    type Block: Block;
    type Grow: Grow;
    type Allocator: Allocator;

    const BASE_OFFSET: isize;

    fn to_vptr(&self, block: *mut Self::Block) -> usize;

    fn offset(&self, offset: isize) -> *mut u8;

    fn head_ptr(&self) -> *mut u8;

    fn base_ptr(&self) -> *mut u8;

    fn tail_ptr(&self) -> *mut u8;

    fn end_ptr(&self) -> *mut u8;

    fn base_size(&self) -> usize;

    fn capacity(&self) -> usize;

    fn size(&self) -> usize;

    fn take(self) -> (*const u8, *const u8);

    fn truncate(&mut self, by: usize) -> TruncResult;

    fn allocate(&mut self, size: usize) -> *mut Self::Block;

    fn append(&mut self, size: usize) -> *mut Self::Block;

    fn reallocate(&mut self, block: &mut Self::Block, size: usize) -> *mut Self::Block;

    fn deallocate(&mut self, block: &mut Self::Block);
}

pub trait BuilderRef<'a, B: Builder> {
    fn builder_mut(&mut self) -> *mut B;
}

pub trait BuilderRoot<'a, B: Builder>: BuilderRef<'a, B> {
    // type Completed: data::Record<'a>;
    // type Message: Message<'a>;
    const SIZE: usize;

    fn new(builder: B) -> Self;
}

pub struct Slice<'a, B, T> {
    pub owner: &'a mut B,
    pub base: T,
}

impl <'a, A, B> Slice<'a, A, B> {
    pub fn new(p: &'a mut A, b: B) -> Self {
        Self { owner: p, base: b }
    }
}

impl <'a, A, B> Deref for Slice<'a, A, B> {
    type Target = B;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl <'a, A, B> DerefMut for Slice<'a, A, B> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

// pub struct Allocation<'a, B: Builder> {
//     b: *mut B,
//     p: *mut B::Block,
//     _p: PhantomData<(&'a ())>
// }
//
// impl<'a, B: Builder> Allocation<'a, B> {
//     fn new(b: *mut B, p: *mut B::Block) -> Self {
//         Self { b, p }
//     }
//     fn reallocate(&mut self) {}
// }

pub struct MessageBuilder<'a,
    B: Builder,
    R: BuilderRoot<'a, B>,
> {
    r: R,
    _phantom: PhantomData<(&'a (), B, R)>,
}

impl<'a,
    B: Builder,
    R: BuilderRoot<'a, B>,
> MessageBuilder<'a, B, R> {
    // const INITIAL_SIZE: usize = B::Header::SIZE as usize + R::SIZE;
    // const DEREF_BASE_OFFSET: isize = B::Header::SIZE - B::Block::SIZE_BYTES;
    // const BASE_OFFSET: isize = B::Header::SIZE;
    // const BASE_SIZE: isize = R::SIZE as isize;

    #[inline(always)]
    pub fn new(builder: B) -> Self {
        Self {
            r: R::new(builder),
            _phantom: PhantomData,
        }
    }

    // #[inline(always)]
    // pub fn base(&'a mut self) -> &mut R {
    //     &mut self.r
    // }

    // pub fn build<F>(mut self, mut f: F)
    //                 -> Option<message::Flex<'a, B::Header, R::Completed, B::Allocator>>
    //     where F: FnMut(&mut R) {
    //     f(&mut self, self.deref_mut());
    //     Some(self.finish())
    // }

    // pub fn finish(mut self) -> message::Flex<'a, B::Header, R::Completed, B::Allocator> {
    //     let (head, base, tail, end) = unsafe { &mut (*self.builder_mut()) }.take();
    //     mem::forget(self);
    //     message::Flex::from_builder(head, end)
    // }
}

impl<'a,
    B: Builder,
    R: BuilderRoot<'a, B>,
> Deref for MessageBuilder<'a, B, R> {
    type Target = R;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.r
    }
}

impl<'a,
    B: Builder,
    R: BuilderRoot<'a, B>,
> DerefMut for MessageBuilder<'a, B, R> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.r
    }
}

pub struct Appender<'a,
    H: Header,
    G: Grow = Doubled,
    A: Allocator = Global
> {
    root: *mut u8,
    end: *mut u8,
    trash: usize,
    _p: PhantomData<(&'a (), H, G, A)>,
}

impl<'a, H, G, A> Appender<'a, H, G, A>
    where
        H: Header,
        G: Grow,
        A: Allocator {
    pub fn new<R>(extra: usize) -> Option<MessageBuilder<'a, Self, R>>
        where
            R: BuilderRoot<'a, Self> {
        let size = H::SIZE as usize + R::SIZE;
        let capacity = size + extra;
        let p = unsafe { A::allocate(capacity) };
        if p == ptr::null_mut() {
            return None;
        }
        let m = MessageBuilder::new(unsafe {
            Self {
                root: p,
                end: p.offset(capacity as isize),
                trash: 0usize,
                _p: PhantomData,
            }
        });
        Some(m)
    }

    pub fn wrap(root: *mut u8, end: *mut u8) -> Self {
        Self { root, end, trash: 0, _p: PhantomData }
    }
}

impl<'a, H, G, A> Drop for Appender<'a, H, G, A>
    where
        H: Header,
        G: Grow,
        A: Allocator {
    fn drop(&mut self) {
        A::deallocate(self.root, self.capacity());
    }
}

impl<'a, H, G, A> Builder for Appender<'a, H, G, A>
    where
        H: Header, G: Grow, A: Allocator {
    type Header = H;
    type Block = H::Block;
    type Grow = G;
    type Allocator = A;

    const BASE_OFFSET: isize = H::SIZE - H::Block::SIZE_BYTES as isize;

    #[inline(always)]
    fn to_vptr(&self, block: *mut Self::Block) -> usize {
        Self::Block::offset(self.root, block) as usize
    }

    #[inline(always)]
    fn offset(&self, offset: isize) -> *mut u8 {
        unsafe { self.root.offset(offset) }
    }

    #[inline(always)]
    fn head_ptr(&self) -> *mut u8 {
        self.root
    }

    #[inline(always)]
    fn base_ptr(&self) -> *mut u8 {
        unsafe { self.root.offset(H::SIZE) }
    }

    #[inline(always)]
    fn tail_ptr(&self) -> *mut u8 {
        unsafe { self.root.offset(self.size() as isize) }
    }

    #[inline(always)]
    fn end_ptr(&self) -> *mut u8 {
        self.end
    }

    #[inline(always)]
    fn base_size(&self) -> usize {
        (unsafe { &mut *(self.root as *mut H) }).base_size()
    }

    #[inline(always)]
    fn capacity(&self) -> usize {
        unsafe { self.end as usize - self.root as usize }
    }

    #[inline(always)]
    fn size(&self) -> usize {
        H::raw_size(self.root)
    }

    #[inline(always)]
    fn take(self) -> (*const u8, *const u8) {
        let root = self.root;
        let end = self.end;
        mem::forget(self);
        (root, end)
    }

    fn truncate(&mut self, by: usize) -> TruncResult {
        let current_capacity = self.capacity();
        let new_capacity = G::calc(current_capacity, by);

        if new_capacity > H::Block::SIZE_LIMIT {
            return TruncResult::Overflow;
        }
        let existing_size = self.size();

        // can't shrink
        if new_capacity < current_capacity {
            return TruncResult::Underflow;
        }

        // try to reallocate
        let (new_buffer, new_buffer_capacity) = A::reallocate(self.root, new_capacity);
        if new_buffer == ptr::null_mut() {
            return TruncResult::OutOfMemory;
        }

        // in place reallocation?
        if self.root == new_buffer {
            // ensure the end is correctly set
            self.end = unsafe { new_buffer.offset(new_capacity as isize) };
            return TruncResult::Success;
        }

        self.root = new_buffer;
        self.end = unsafe { new_buffer.offset(new_capacity as isize) };
        TruncResult::Success
    }

    #[inline(always)]
    fn allocate(&mut self, size: usize) -> *mut Self::Block {
        self.append(size)
    }

    fn append(&mut self, size: usize) -> *mut Self::Block {
        let current_size = self.size();
        let new_size = current_size + Self::Block::OVERHEAD as usize;
        let mut new_tail = unsafe { self.tail_ptr().offset(Self::Block::OVERHEAD + size as isize) };

        if new_tail > self.end {
            match self.truncate(size + Self::Block::OVERHEAD as usize) {
                TruncResult::Success => {
                    new_tail = unsafe { self.tail_ptr().offset(Self::Block::OVERHEAD + size as isize) };
                }
                _ => return ptr::null_mut(),
            }
        }

        let block = Self::Block::from_ptr(self.tail_ptr());

        // update entire message size
        H::raw_set_size(self.root, new_size);
        // set the block's size
        block.set_size_usize(size);

        unsafe { block as *mut Self::Block }
    }

    fn reallocate(&mut self, block: &mut Self::Block, size: usize) -> *mut Self::Block {
        if block.is_free() {
            return ptr::null_mut();
        }
        let current_size = self.size();
        let offset = Self::Block::offset(self.root, block);

        // out of bounds?
        if offset < 0 || offset > current_size as isize - Self::Block::MIN_SIZE {
            return ptr::null_mut();
        }

        let current_block_size = block.size_usize();

        // Is it the last allocation?
        if current_size == offset as usize + Self::Block::OVERHEAD as usize + current_block_size {
            let new_size = current_size - current_block_size + size;

            if current_block_size >= size {
                H::raw_set_size(self.root, new_size);
                return block;
            }

            let mut new_tail = unsafe {
                self.tail_ptr().offset(Self::Block::OVERHEAD + size as isize)
            };
            // Resize required?
            if new_tail > self.end {
                match self.truncate(size + Self::Block::OVERHEAD as usize) {
                    TruncResult::Success => {
                        new_tail = unsafe { self.tail_ptr().offset(Self::Block::OVERHEAD + size as isize) };
                    }
                    _ => return ptr::null_mut(),
                }

                H::raw_set_size(self.root, new_size);
                let block = Self::Block::from_ptr(
                    unsafe { self.root.offset(offset) }
                );
                block.set_size_usize(size);
                return block;
            } else {
                H::raw_set_size(self.root, new_size);
                block.set_size_usize(size);
                return block;
            }
        }

        if current_block_size >= size {
            self.trash += current_block_size - size;
            return block;
        }

        let new_block = self.append(size);
        if new_block == ptr::null_mut() {
            return ptr::null_mut();
        }
        // deref block based on original offset since append may have to create a new allocation
        let block = Self::Block::from_ptr(unsafe { self.root.offset(offset) });
        unsafe {
            ptr::copy_nonoverlapping(
                block.data_ptr(),
                (&*new_block).data_ptr() as *mut u8,
                core::cmp::min(size, current_block_size));
        }

        block.set_free();
        self.trash += current_block_size + Self::Block::OVERHEAD as usize;

        new_block
    }

    fn deallocate(&mut self, block: &mut Self::Block) {
        let current_size = self.size();
        let offset = Self::Block::offset(self.root, block);

        // out of bounds?
        if offset < 0 || offset > current_size as isize - Self::Block::MIN_SIZE {
            return;
        }

        if block.is_free() {
            return;
        }

        let block_size = block.size_usize();
        if current_size == offset as usize + Self::Block::OVERHEAD as usize + block_size {
            H::raw_set_size(self.root, offset as usize);
            return;
        }

        self.trash += block_size + Self::Block::OVERHEAD as usize;
    }
}


#[cfg(test)]
mod tests {
    use crate::hash::hash_default;
    use crate::header::Flex16;
    use crate::message::{Provider, Safe, Unsafe, UnsafeProvider};

    use super::*;

    // pub trait Order<'a> {
    //     type Price: Price<'a>;
    //
    //     fn id(&self) -> u64;
    //
    //     fn set_id(&mut self, value: u64) -> &mut Self;
    //
    //     fn price(&'a self) -> Self::Price;
    //
    //     fn price_mut(&mut self) -> (Self::Price, &mut Self);
    //
    //     fn set_price<'b>(&'a mut self, price: impl Price<'b>) -> &'a mut Self;
    // }
    //
    // pub trait OrderBuilder<'a>: Order<'a> {
    //     fn set_client_id(&mut self, value: &str) -> &mut Self;
    // }
    //
    // pub trait Price<'a> {
    //     fn wap_hash(&self) -> u64;
    //
    //     fn copy_to<'b>(&self, to: &mut impl Price<'b>);
    //
    //     fn open(&self) -> f64;
    //     fn high(&self) -> f64;
    //     fn low(&self) -> f64;
    //     fn close(&self) -> f64;
    //
    //     fn set_open(&mut self, value: f64) -> &mut Self;
    //     fn set_high(&mut self, value: f64) -> &mut Self;
    //     fn set_low(&mut self, value: f64) -> &mut Self;
    //     fn set_close(&mut self, value: f64) -> &mut Self;
    // }
    //
    //
    // /////////////////////////////////////////////////////////////////////////////
    // // Order - Layout
    // /////////////////////////////////////////////////////////////////////////////
    // #[repr(C, packed)]
    // pub struct OrderData {
    //     id: u64,
    //     client_id: [u8; 10],
    //     price: PriceData,
    // }
    //
    // //////////////////////////////////////////////////////////////////////////////
    // // OrderMessage
    // //////////////////////////////////////////////////////////////////////////////
    //
    // pub struct OrderMessage<'a, H: 'a + Header = Flex16>(PhantomData<(&'a (), H)>);
    //
    // impl<'a, H: 'a + Header> Message<'a> for OrderMessage<'a, H> {
    //     type Header = H;
    //     type Layout = OrderData;
    //     type Safe = OrderProvider<'a, Self>;
    //     type Unsafe = OrderUnsafeProvider<'a, Self>;
    //
    //     const SIZE: usize = mem::size_of::<Self::Layout>();
    //     const INITIAL_SIZE: usize = H::SIZE as usize + Self::SIZE;
    //     const ZERO_OFFSET: isize = H::SIZE - H::Block::SIZE_BYTES;
    //     const BASE_OFFSET: isize = H::SIZE;
    //     const BASE_SIZE: isize = Self::SIZE as isize;
    // }
    //
    // impl<'a> OrderMessage<'a, Flex16> {
    //     pub fn new() -> Option<Safe<'a, OrderMessage<'a, Flex16>>> {
    //         Safe::new()
    //     }
    //
    //     pub fn new_unsafe() -> Option<Unsafe<'a, OrderMessage<'a, Flex16>>> {
    //         Unsafe::new()
    //     }
    //
    //     fn new_builder() -> Option<MessageBuilder<'a,
    //         Appender::<'a, Flex16, Doubled, Global>,
    //         OrderBuilderRoot<'a, Flex16, Appender::<'a, Flex16, Doubled, Global>>>> {
    //         Appender::<'a, Flex16, Doubled, Global>::new
    //             ::<OrderBuilderRoot<'a, Flex16, Appender::<'a, Flex16, Doubled, Global>>>
    //             (32)
    //     }
    //
    //     fn copy<'b>(src: &impl Order<'a>, dst: &mut impl OrderBuilder<'b>) {
    //         // dst.set_id(src.id()).
    //         //     set_client_id(src.client_id());
    //     }
    // }
    //
    // impl<'a, H: Header + Header16> OrderMessage<'a, H> {
    //     pub fn new_with_header() -> Option<Safe<'a, OrderMessage<'a, H>>> {
    //         Safe::new()
    //     }
    //
    //     pub fn new_unsafe_with_header() -> Option<Unsafe<'a, OrderMessage<'a, H>>> {
    //         Unsafe::new()
    //     }
    // }
    //
    // ////////////////////////////////////////////////////////////////////////////////////////////////
    // // OrderSafe
    // ////////////////////////////////////////////////////////////////////////////////////////////////
    //
    // pub struct OrderProvider<'a, R: 'a + Message<'a>> {
    //     base: *const OrderData,
    //     _p: PhantomData<(&'a (), R)>,
    // }
    //
    // impl<'a, R: 'a + Message<'a>> Provider<'a> for OrderProvider<'a, R> {
    //     fn new(base: *const u8) -> Self {
    //         Self { base: unsafe { base as *const OrderData }, _p: PhantomData }
    //     }
    //
    //     fn base_ptr(&self) -> *const u8 {
    //         unsafe { self.base as *const u8 }
    //     }
    // }
    //
    // impl<'a, R: 'a + Message<'a>> Order<'a> for OrderProvider<'a, R> {
    //     type Price = PriceProvider<'a, R>;
    //
    //     fn id(&self) -> u64 {
    //         unsafe { (&*(self.base)).id }
    //     }
    //
    //     fn set_id(&mut self, value: u64) -> &mut Self {
    //         unsafe { (&mut *(self.base as *mut OrderData)).id = value; }
    //         self
    //     }
    //
    //     fn price(&'a self) -> Self::Price {
    //         Self::Price::new(unsafe { (self.base as *const u8).offset(10) })
    //     }
    //
    //     fn price_mut(&mut self) -> (Self::Price, &mut Self) {
    //         (Self::Price::new(unsafe { (self.base as *const u8).offset(10) }), self)
    //     }
    //
    //     fn set_price<'b>(&'a mut self, price: impl Price<'b>) -> &'a mut Self {
    //         self
    //     }
    // }
    //
    // ////////////////////////////////////////////////////////////////////////////////////////////////
    // // OrderUnsafe
    // ////////////////////////////////////////////////////////////////////////////////////////////////
    // pub struct OrderUnsafeProvider<'a, R: 'a + Message<'a>> {
    //     base: *const OrderData,
    //     bounds: *const u8,
    //     _p: PhantomData<(&'a (), R)>,
    // }
    //
    // impl<'a, R: 'a + Message<'a>> UnsafeProvider<'a> for OrderUnsafeProvider<'a, R> {
    //     fn new(base: *const u8, bounds: *const u8) -> Self {
    //         Self { base: unsafe { base as *const OrderData }, bounds, _p: PhantomData }
    //     }
    //
    //     fn base_ptr(&self) -> *const u8 {
    //         unsafe { self.base as *const u8 }
    //     }
    // }
    //
    //
    // pub struct OrderNestedProvider<'a, R: 'a + Message<'a>> {
    //     base: *const OrderData,
    //     root: *const u8,
    //     _p: PhantomData<(&'a (), R)>,
    // }
    //
    // pub struct OrderUnsafeNestedProvider<'a, R: 'a + Message<'a>> {
    //     base: *const OrderData,
    //     bounds: *const u8,
    //     root: *const u8,
    //     _p: PhantomData<(&'a (), R)>,
    // }
    //
    // impl<'a, R: 'a + Message<'a>> OrderNestedProvider<'a, R> {
    //     fn new(base: *const u8, root: *const u8) -> Self {
    //         Self { base: unsafe { base as *const OrderData }, root, _p: PhantomData }
    //     }
    // }
    //
    // impl<'a, R: 'a + Message<'a>> OrderUnsafeNestedProvider<'a, R> {
    //     fn new(base: *const u8, bounds: *const u8, root: *const u8) -> Self {
    //         Self { base: unsafe { base as *const OrderData }, bounds, root, _p: PhantomData }
    //     }
    // }
    //
    // pub struct PriceMessage<'a, H: 'a + Header>(PhantomData<(&'a (), H)>);
    //
    // impl<'a, H: 'a + Header> Message<'a> for PriceMessage<'a, H> {
    //     type Header = H;
    //     type Layout = PriceData;
    //     type Safe = PriceProvider<'a, Self>;
    //     type Unsafe = PriceUnsafeProvider<'a, Self>;
    //
    //     const SIZE: usize = mem::size_of::<Self::Layout>();
    //     const INITIAL_SIZE: usize = H::SIZE as usize + Self::SIZE;
    //     const ZERO_OFFSET: isize = H::SIZE - H::Block::SIZE_BYTES;
    //     const BASE_OFFSET: isize = H::SIZE;
    //     const BASE_SIZE: isize = Self::SIZE as isize;
    // }
    //
    // impl<'a> PriceMessage<'a, Fixed16> {
    //     pub fn new() -> Option<Safe<'a, PriceMessage<'a, Fixed16>>> {
    //         Safe::new()
    //     }
    // }
    //
    // pub struct PriceProvider<'a, R: 'a + Message<'a>> {
    //     base: *const PriceData,
    //     _p: PhantomData<(&'a (), R)>,
    // }
    //
    // impl<'a, R: 'a + Message<'a>> Provider<'a> for PriceProvider<'a, R> {
    //     fn new(base: *const u8) -> Self {
    //         Self { base: unsafe { base as *const PriceData }, _p: PhantomData }
    //     }
    //
    //     fn base_ptr(&self) -> *const u8 {
    //         unsafe { self.base as *const u8 }
    //     }
    // }
    //
    // impl<'a, M: 'a + Message<'a>> Price<'a> for PriceProvider<'a, M> {
    //     #[inline(always)]
    //     fn wap_hash(&self) -> u64 {
    //         hash_default(unsafe { self.base as *const u8 },
    //                      mem::size_of::<PriceData>() as u64)
    //     }
    //
    //     #[inline(always)]
    //     fn copy_to<'b>(&self, to: &mut impl Price<'b>) {
    //         copy_price(self, to);
    //     }
    //
    //     #[inline(always)]
    //     fn open(&self) -> f64 {
    //         unsafe { (&*self.base).open() }
    //     }
    //
    //     #[inline(always)]
    //     fn high(&self) -> f64 {
    //         unsafe { (&*self.base).high() }
    //     }
    //
    //     #[inline(always)]
    //     fn low(&self) -> f64 {
    //         unsafe { (&*self.base).low() }
    //     }
    //
    //     #[inline(always)]
    //     fn close(&self) -> f64 {
    //         unsafe { ((&*self.base)).close() }
    //     }
    //
    //     #[inline(always)]
    //     fn set_open(&mut self, value: f64) -> &mut Self {
    //         unsafe { (&mut *(self.base as *mut PriceData)).set_open(value); }
    //         self
    //     }
    //
    //     #[inline(always)]
    //     fn set_high(&mut self, value: f64) -> &mut Self {
    //         unsafe { (&mut *(self.base as *mut PriceData)).set_open(value); }
    //         self
    //     }
    //
    //     #[inline(always)]
    //     fn set_low(&mut self, value: f64) -> &mut Self {
    //         unsafe { (&mut *(self.base as *mut PriceData)).set_low(value); }
    //         self
    //     }
    //
    //     #[inline(always)]
    //     fn set_close(&mut self, value: f64) -> &mut Self {
    //         unsafe { (&mut *(self.base as *mut PriceData)).set_close(value); }
    //         self
    //     }
    // }
    //
    //
    // pub struct PriceUnsafeProvider<'a, R: 'a + Message<'a>> {
    //     base: *const PriceData,
    //     bounds: *const u8,
    //     _p: PhantomData<(&'a (), R)>,
    // }
    //
    // impl<'a, R: 'a + Message<'a>> UnsafeProvider<'a> for PriceUnsafeProvider<'a, R> {
    //     fn new(base: *const u8, bounds: *const u8) -> Self {
    //         Self { base: unsafe { base as *const PriceData }, bounds, _p: PhantomData }
    //     }
    //
    //     fn base_ptr(&self) -> *const u8 {
    //         unsafe { self.base as *const u8 }
    //     }
    // }
    //
    // impl<'a, M: 'a + Message<'a>> Price<'a> for PriceUnsafeProvider<'a, M> {
    //     #[inline(always)]
    //     fn wap_hash(&self) -> u64 {
    //         hash_default(unsafe { self.base as *const u8 },
    //                      mem::size_of::<PriceData>() as u64)
    //     }
    //
    //     #[inline(always)]
    //     fn copy_to<'b>(&self, to: &mut impl Price<'b>) {
    //         copy_price(self, to);
    //     }
    //
    //     #[inline(always)]
    //     fn open(&self) -> f64 {
    //         unsafe {
    //             if self.bounds.offset(8) as usize > self.bounds as usize { 0f64 } else { (&*self.base).open() }
    //         }
    //     }
    //
    //     #[inline(always)]
    //     fn high(&self) -> f64 {
    //         unsafe {
    //             if self.bounds.offset(16) as usize > self.bounds as usize { 0f64 } else { (&*self.base).close() }
    //         }
    //     }
    //
    //     #[inline(always)]
    //     fn low(&self) -> f64 {
    //         unsafe {
    //             if self.bounds.offset(24) as usize > self.bounds as usize { 0f64 } else { (&*self.base).low() }
    //         }
    //     }
    //
    //     #[inline(always)]
    //     fn close(&self) -> f64 {
    //         unsafe {
    //             if self.bounds.offset(32) as usize > self.bounds as usize { 0f64 } else { (&*self.base).close() }
    //         }
    //     }
    //
    //     #[inline(always)]
    //     fn set_open(&mut self, value: f64) -> &mut Self {
    //         unsafe {
    //             if self.bounds.offset(8) as usize > self.bounds as usize { self } else {
    //                 (&mut *(self.base as *mut PriceData)).set_open(value);
    //                 self
    //             }
    //         }
    //     }
    //
    //     #[inline(always)]
    //     fn set_high(&mut self, value: f64) -> &mut Self {
    //         unsafe {
    //             if self.bounds.offset(16) as usize > self.bounds as usize { self } else {
    //                 (&mut *(self.base as *mut PriceData)).set_high(value);
    //                 self
    //             }
    //         }
    //     }
    //
    //     #[inline(always)]
    //     fn set_low(&mut self, value: f64) -> &mut Self {
    //         unsafe {
    //             if self.bounds.offset(24) as usize > self.bounds as usize { self } else {
    //                 (&mut *(self.base as *mut PriceData)).set_low(value);
    //                 self
    //             }
    //         }
    //     }
    //
    //     #[inline(always)]
    //     fn set_close(&mut self, value: f64) -> &mut Self {
    //         unsafe {
    //             if self.bounds.offset(32) as usize > self.bounds as usize { self } else {
    //                 (&mut *(self.base as *mut PriceData)).set_close(value);
    //                 self
    //             }
    //         }
    //     }
    // }
    //
    // pub struct PriceBuilderProvider<'a, B: Builder> {
    //     builder: *const B,
    //     offset: isize,
    //     _p: PhantomData<(&'a ())>,
    // }
    //
    // impl<'a, B: Builder> PriceBuilderProvider<'a, B> {
    //     fn new(base: *const u8, offset: isize, builder: *const B) -> Self {
    //         Self { builder, offset, _p: PhantomData }
    //     }
    //
    //     fn deref(&self) -> &PriceData {
    //         unsafe { &*((&*self.builder).offset(self.offset) as *const PriceData) }
    //     }
    //
    //     fn deref_mut(&mut self) -> &mut PriceData {
    //         unsafe { &mut *((&*self.builder).offset(self.offset) as *mut PriceData) }
    //     }
    // }
    //
    // impl<'a, B: Builder> Price<'a> for PriceBuilderProvider<'a, B> {
    //     #[inline(always)]
    //     fn wap_hash(&self) -> u64 {
    //         hash_default(unsafe { self.builder.offset(self.offset) as *const u8 },
    //                      mem::size_of::<PriceData>() as u64)
    //     }
    //
    //     #[inline(always)]
    //     fn copy_to<'b>(&self, to: &mut impl Price<'b>) {
    //         copy_price(self, to);
    //     }
    //
    //     #[inline(always)]
    //     fn open(&self) -> f64 {
    //         self.deref().open()
    //     }
    //
    //     #[inline(always)]
    //     fn high(&self) -> f64 {
    //         self.deref().high()
    //     }
    //
    //     #[inline(always)]
    //     fn low(&self) -> f64 {
    //         self.deref().low()
    //     }
    //
    //     #[inline(always)]
    //     fn close(&self) -> f64 {
    //         self.deref().close()
    //     }
    //
    //     #[inline(always)]
    //     fn set_open(&mut self, value: f64) -> &mut Self {
    //         self.deref_mut().set_open(value);
    //         self
    //     }
    //
    //     #[inline(always)]
    //     fn set_high(&mut self, value: f64) -> &mut Self {
    //         self.deref_mut().set_high(value);
    //         self
    //     }
    //
    //     #[inline(always)]
    //     fn set_low(&mut self, value: f64) -> &mut Self {
    //         self.deref_mut().set_low(value);
    //         self
    //     }
    //
    //     #[inline(always)]
    //     fn set_close(&mut self, value: f64) -> &mut Self {
    //         self.deref_mut().set_close(value);
    //         self
    //     }
    // }
    //
    //
    // #[inline(always)]
    // fn copy_price<'a, 'b>(from: &impl Price<'a>, to: &mut impl Price<'b>) {
    //     to.set_open(from.open())
    //         .set_high(from.high())
    //         .set_low(from.low())
    //         .set_close(from.close());
    // }
    //
    // #[derive(Copy, Clone, Debug)]
    // #[repr(C, packed)]
    // pub struct PriceData {
    //     open: f64,
    //     high: f64,
    //     low: f64,
    //     close: f64,
    // }
    //
    // impl PriceData {
    //     pub fn new(open: f64, high: f64, low: f64, close: f64) -> Self {
    //         Self { open, high, low, close }
    //     }
    // }
    //
    // impl<'a> Price<'a> for PriceData {
    //     #[inline(always)]
    //     fn wap_hash(&self) -> u64 {
    //         hash_default(unsafe { self as *const Self as *const u8 },
    //                      mem::size_of::<PriceData>() as u64)
    //     }
    //
    //     #[inline(always)]
    //     fn copy_to<'b>(&self, to: &mut impl Price<'b>) {
    //         copy_price(self, to);
    //     }
    //
    //     #[inline(always)]
    //     fn open(&self) -> f64 {
    //         f64::from_bits(f64::to_bits(self.open).to_le())
    //     }
    //     #[inline(always)]
    //     fn high(&self) -> f64 {
    //         f64::from_bits(f64::to_bits(self.high).to_le())
    //     }
    //     #[inline(always)]
    //     fn low(&self) -> f64 {
    //         f64::from_bits(f64::to_bits(self.low).to_le())
    //     }
    //     #[inline(always)]
    //     fn close(&self) -> f64 {
    //         f64::from_bits(f64::to_bits(self.close).to_le())
    //     }
    //
    //     #[inline(always)]
    //     fn set_open(&mut self, value: f64) -> &mut Self {
    //         self.open = f64::from_bits(f64::to_bits(value).to_le());
    //         self
    //     }
    //
    //     #[inline(always)]
    //     fn set_high(&mut self, value: f64) -> &mut Self {
    //         self.close = f64::from_bits(f64::to_bits(value).to_le());
    //         self
    //     }
    //
    //     #[inline(always)]
    //     fn set_low(&mut self, value: f64) -> &mut Self {
    //         self.low = f64::from_bits(f64::to_bits(value).to_le());
    //         self
    //     }
    //
    //     #[inline(always)]
    //     fn set_close(&mut self, value: f64) -> &mut Self {
    //         self.close = f64::from_bits(f64::to_bits(value).to_le());
    //         self
    //     }
    // }
    //
    //
    // //////////////////////////////////////////////////////////////////////////////////////////
    // // OrderBuilder Root
    // //////////////////////////////////////////////////////////////////////////////////////////
    //
    // pub struct OrderBuilderRoot<'a, H: Header, B: Builder> {
    //     builder: B,
    //     _phantom: PhantomData<(&'a (), H, B)>,
    // }
    //
    // impl<'a, H: Header, B: Builder> BuilderRef<'a, B> for OrderBuilderRoot<'a, H, B> {
    //     fn builder_mut(&mut self) -> *mut B {
    //         unsafe { &mut self.builder as *mut B }
    //     }
    // }
    //
    // impl<'a, H: Header, B: Builder> BuilderRoot<'a, B> for OrderBuilderRoot<'a, H, B> {
    //     const SIZE: usize = mem::size_of::<OrderData>();
    //     // type Message = OrderMessage<'a, H>;
    //
    //
    //     fn new(builder: B) -> Self {
    //         Self { builder, _phantom: PhantomData }
    //     }
    // }
    //
    // pub struct Nested<'a>(PhantomData<(&'a ())>);
    //
    // impl<'a> Nested<'a> {
    //     pub fn new() -> Self {
    //         Self(PhantomData)
    //     }
    // }
    //
    // impl<'a, H: Header, B: Builder> OrderBuilderRoot<'a, H, B> {
    //     fn nested<'b>(&'b mut self) -> OrderBuilderNested<'b, B> {
    //         let o = OrderBuilderNested::<'b, B>::new(10, 10 + 32, &mut self.builder);
    //         o
    //     }
    //
    //     fn nested2<'b>(&'b mut self) -> Nested<'b> {
    //         Nested::new()
    //     }
    // }
    //
    // impl<'a, H: Header, B: Builder> Order<'a> for OrderBuilderRoot<'a, H, B> {
    //     type Price = PriceBuilderProvider<'a, B>;
    //
    //     #[inline(always)]
    //     fn id(&self) -> u64 {
    //         self.deref().id
    //     }
    //
    //     #[inline(always)]
    //     fn set_id(&mut self, id: u64) -> &mut Self {
    //         self.deref_mut().id = id;
    //         self
    //     }
    //
    //     fn price(&'a self) -> Self::Price {
    //         todo!()
    //     }
    //
    //     fn price_mut(&mut self) -> (Self::Price, &mut Self) {
    //         todo!()
    //     }
    //
    //     fn set_price<'b>(&'a mut self, price: impl Price<'b>) -> &'a mut Self {
    //         todo!()
    //     }
    // }
    //
    // impl<'a, H: Header, B: Builder> OrderBuilder<'a> for OrderBuilderRoot<'a, H, B> {
    //     #[inline(always)]
    //     fn set_client_id(&mut self, value: &str) -> &mut Self {
    //         // let block = self.builder().append(value.len());
    //         self
    //     }
    // }
    //
    // impl<'a, H: Header, B: Builder> OrderBuilderRoot<'a, H, B> {
    //     fn deref(&self) -> &OrderData {
    //         unsafe { &*(self.builder.offset(B::BASE_OFFSET) as *const OrderData) }
    //     }
    //     fn deref_mut(&mut self) -> &mut OrderData {
    //         unsafe { &mut *(self.builder.offset(B::BASE_OFFSET) as *mut OrderData) }
    //     }
    // }
    //
    // //////////////////////////////////////////////////////////////////////////////////////////
    // // OrderBuilder Nested
    // //////////////////////////////////////////////////////////////////////////////////////////
    //
    // pub struct OrderBuilderNested<'a, B: Builder> {
    //     builder: *mut B,
    //     offset: isize,
    //     _phantom: PhantomData<(&'a (), B)>,
    // }
    //
    // impl<'a, B: Builder> BuilderRef<'a, B> for OrderBuilderNested<'a, B> {
    //     fn builder_mut(&mut self) -> *mut B {
    //         self.builder
    //     }
    // }
    //
    // impl<'a, B: Builder> OrderBuilderNested<'a, B> {
    //     fn new(offset: isize, bounds: isize, builder: *mut B) -> Self {
    //         Self {
    //             builder,
    //             offset,
    //             _phantom: PhantomData,
    //         }
    //     }
    // }
    //
    // impl<'a, B: Builder> OrderBuilderNested<'a, B> {
    //     fn deref(&self) -> &OrderData {
    //         unsafe { &*((&*self.builder).offset(self.offset) as *mut OrderData) }
    //     }
    //     fn deref_mut(&mut self) -> &mut OrderData {
    //         unsafe { &mut *((&*self.builder).offset(self.offset) as *mut OrderData) }
    //     }
    // }
    //
    // impl<'a, B: Builder> Order<'a> for OrderBuilderNested<'a, B> {
    //     type Price = PriceBuilderProvider<'a, B>;
    //
    //     #[inline(always)]
    //     fn id(&self) -> u64 {
    //         self.deref().id
    //     }
    //
    //     #[inline(always)]
    //     fn set_id(&mut self, id: u64) -> &mut Self {
    //         self.deref_mut().id = id;
    //         self
    //     }
    //
    //     fn price(&'a self) -> Self::Price {
    //         todo!()
    //     }
    //
    //     fn price_mut(&mut self) -> (Self::Price, &mut Self) {
    //         todo!()
    //     }
    //
    //     fn set_price<'b>(&'a mut self, price: impl Price<'b>) -> &'a mut Self {
    //         todo!()
    //     }
    // }
    //
    // impl<'a, B: Builder> OrderBuilder<'a> for OrderBuilderNested<'a, B> {
    //     #[inline(always)]
    //     fn set_client_id(&mut self, value: &str) -> &mut Self {
    //         // let block = self.builder().append(value.len());
    //         self
    //     }
    // }
    //
    //
    // // pub type OrderType = impl Order + OrderBuilder;
    //
    // fn process<'a>(order: &mut impl OrderBuilder<'a>) {
    //     println!("order");
    // }
    //
    // fn process_builder<'a>(order: &mut impl OrderBuilder<'a>) {}
    //
    // // fn build<'a>() -> impl OrderBuilder<'a> {
    // fn build() {
    // // fn build<'a>() -> Nested<'a> {
    //     // fn build<'a>() -> &'a mut OrderBuilder16<'a, Appender<Fixed16>> {
    //     let mut message = OrderMessage::new_builder().unwrap();
    //     message.set_id(11);
    //     message.set_id(12);
    //
    //
    //     // message.nested().id();
    //     // message.nested().id();
    //
    //     // let order = message.finish();
    //     println!("id: {:}", message.id());
    //
    //     let n = message.nested();
    //     let n = message.nested2();
    //     message.nested().id();
    //     // n
    //     // n
    // }
    //
    // #[test]
    // fn builder_test() {
    //     build();
    //     let mut message = OrderMessage::new().unwrap();
    //     // let mut order = message.base();
    //     println!("id: {:}", message.id());
    //     // message.build(|b| {
    //     //     // let o = b.unsafe_mut();
    //     //     // b.set_id(12);
    //     //     process(b);
    //     // });
    //     // let m = message.finish();
    //
    //     println!("done");
    // }
}