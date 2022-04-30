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

pub trait Record<'a> {
    type Builder: Builder;

    const SIZE: usize;
}

pub trait Root<'a, B: Builder>: Record<'a, Builder=B> + BuilderRef<'a, B> {
    // type Completed: data::Record<'a>;

    fn new(builder: B) -> Self;
}

pub trait Nested<'a>: Record<'a> {
    fn new(offset: isize, bounds: isize, builder: *mut Self::Builder) -> Self;
}

pub struct Allocation<B: Builder> {
    b: *mut B,
    p: *mut B::Block,
}

impl<B: Builder> Allocation<B> {
    fn new(b: *mut B, p: *mut B::Block) -> Self {
        Self { b, p }
    }
    fn reallocate(&mut self) {}
}

pub struct Message<'a,
    B: Builder,
    R: 'a + Root<'a, B>,
> {
    r: R,
    _phantom: PhantomData<(&'a (), B, R)>,
}

impl<'a,
    B: Builder,
    R: 'a + Root<'a, B>,
> Message<'a, B, R> {
    const INITIAL_SIZE: usize = B::Header::SIZE as usize + R::SIZE;
    const DEREF_BASE_OFFSET: isize = B::Header::SIZE - B::Block::SIZE_BYTES;
    const BASE_OFFSET: isize = B::Header::SIZE;
    const BASE_SIZE: isize = R::SIZE as isize;

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
    R: 'a + Root<'a, B>,
> Deref for Message<'a, B, R> {
    type Target = R;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.r
    }
}

impl<'a,
    B: Builder,
    R: 'a + Root<'a, B>,
> DerefMut for Message<'a, B, R> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.r
    }
}

pub struct Appender<
    H: Header,
    G: Grow = Doubled,
    A: Allocator = Global
> {
    root: *mut u8,
    end: *mut u8,
    trash: usize,
    _p: PhantomData<(H, G, A)>,
}

impl<H, G, A> Appender<H, G, A>
    where
        H: Header,
        G: Grow,
        A: Allocator {
    pub fn new<'a, R>(extra: usize) -> Option<Message<'a, Self, R>>
        where
            R: 'a + Root<'a, Self> {
        let size = H::SIZE as usize + R::SIZE;
        let capacity = size + extra;
        let p = unsafe { A::allocate(capacity) };
        if p == ptr::null_mut() {
            return None;
        }
        let m = Message::new(unsafe {
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

impl<H, G, A> Drop for Appender<H, G, A>
    where
        H: Header,
        G: Grow,
        A: Allocator {
    fn drop(&mut self) {
        A::deallocate(self.root, self.capacity());
    }
}

impl<H, G, A> Builder for Appender<H, G, A>
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
    use crate::header::Flex16;

    use super::*;

    pub trait Order<'a> {
        fn id(&self) -> u64;

        fn set_id(&mut self, id: u64) -> &mut Self;

        fn client_id(&self) -> &str;
    }

    pub trait OrderBuilder<'a>: Order<'a> {
        fn set_client_id(&mut self, value: &str) -> &mut Self;
    }

    #[repr(C, packed)]
    pub struct Order16 {
        id: u64,
        client_id: [u8; 10],
    }

    impl Order16 {
        pub fn id(&self) -> u64 {
            return self.id;
        }
    }

    pub struct OrderSafe<'a> {
        p: *const Order16,
        _p: PhantomData<(&'a ())>,
    }

    //////////////////////////////////////////////////////////////////////////////////////////
    // OrderBuilder Root
    //////////////////////////////////////////////////////////////////////////////////////////

    pub struct OrderBuilderRoot<'a, B: Builder> {
        builder: B,
        // owns the builder
        _phantom: PhantomData<(&'a ())>,
    }

    impl<'a, B: Builder> BuilderRef<'a, B> for OrderBuilderRoot<'a, B> {
        fn builder_mut(&mut self) -> *mut B {
            unsafe { &mut self.builder as *mut B }
        }
    }

    impl<'a, B: Builder> Record<'a> for OrderBuilderRoot<'a, B> {
        type Builder = B;

        const SIZE: usize = mem::size_of::<Order16>() as usize;
    }

    impl<'a, B: Builder> Root<'a, B> for OrderBuilderRoot<'a, B> {
        fn new(builder: Self::Builder) -> Self {
            Self { builder, _phantom: PhantomData }
        }
    }

    impl<'a, B: Builder> Order<'a> for OrderBuilderRoot<'a, B> {
        #[inline(always)]
        fn id(&self) -> u64 {
            self.deref().id
        }

        #[inline(always)]
        fn set_id(&mut self, id: u64) -> &mut Self {
            self.deref_mut().id = id;
            self
        }

        #[inline(always)]
        fn client_id(&self) -> &str {
            ""
        }
    }

    impl<'a, B: Builder> OrderBuilder<'a> for OrderBuilderRoot<'a, B> {
        #[inline(always)]
        fn set_client_id(&mut self, value: &str) -> &mut Self {
            // let block = self.builder().append(value.len());
            self
        }
    }

    impl<'a, B: Builder> OrderBuilderRoot<'a, B> {
        fn deref(&self) -> &Order16 {
            unsafe { &*(self.builder.offset(B::BASE_OFFSET) as *const Order16) }
        }
        fn deref_mut(&mut self) -> &mut Order16 {
            unsafe { &mut *(self.builder.offset(B::BASE_OFFSET) as *mut Order16) }
        }
    }

    //////////////////////////////////////////////////////////////////////////////////////////
    // OrderBuilder Nested
    //////////////////////////////////////////////////////////////////////////////////////////

    pub struct OrderBuilderNested<'a, B: Builder> {
        builder: *mut B,
        offset: isize,
        _phantom: PhantomData<(&'a ())>,
    }

    impl<'a, B: Builder> Record<'a> for OrderBuilderNested<'a, B> {
        type Builder = B;

        const SIZE: usize = mem::size_of::<Order16>() as usize;
    }

    impl<'a, B: Builder> BuilderRef<'a, B> for OrderBuilderNested<'a, B> {
        fn builder_mut(&mut self) -> *mut B {
            self.builder
        }
    }

    impl<'a, B: Builder> OrderBuilderNested<'a, B> {
        fn new(offset: isize, bounds: isize, builder: *mut B) -> Self {
            Self {
                builder,
                offset,
                _phantom: PhantomData,
            }
        }
    }

    impl<'a, B: Builder> OrderBuilderNested<'a, B> {
        fn deref(&self) -> &Order16 {
            unsafe { &*((&*self.builder).offset(self.offset) as *mut Order16) }
        }
        fn deref_mut(&mut self) -> &mut Order16 {
            unsafe { &mut *((&*self.builder).offset(self.offset) as *mut Order16) }
        }
    }

    impl<'a, B: Builder> Order<'a> for OrderBuilderNested<'a, B> {
        #[inline(always)]
        fn id(&self) -> u64 {
            self.deref().id
        }

        #[inline(always)]
        fn set_id(&mut self, id: u64) -> &mut Self {
            self.deref_mut().id = id;
            self
        }

        #[inline(always)]
        fn client_id(&self) -> &str {
            ""
        }
    }

    impl<'a, B: Builder> OrderBuilder<'a> for OrderBuilderNested<'a, B> {
        #[inline(always)]
        fn set_client_id(&mut self, value: &str) -> &mut Self {
            // let block = self.builder().append(value.len());
            self
        }
    }

    pub struct OrderMessage;

    impl OrderMessage {
        fn new<'a>() -> Option<Message<'a,
            Appender::<Flex16, Doubled, Global>,
            OrderBuilderRoot<'a, Appender::<Flex16, Doubled, Global>>>> {
            Appender::<Flex16, Doubled, Global>::new
                ::<OrderBuilderRoot<'a, Appender::<Flex16, Doubled, Global>>>
                (32)
        }

        fn copy<'a, 'b>(src: &impl Order<'a>, dst: &mut impl OrderBuilder<'b>) {
            dst.set_id(src.id()).
                set_client_id(src.client_id());
        }
    }

    // pub type OrderType = impl Order + OrderBuilder;

    fn process<'a>(order: &mut impl OrderBuilder<'a>) {
        println!("order");
    }

    fn process_builder<'a>(order: &mut impl OrderBuilder<'a>) {}

    // fn build() -> impl BTrait {
    fn build() {
        // fn build<'a>() -> &'a mut OrderBuilder16<'a, Appender<Fixed16>> {
        let mut message = OrderMessage::new().unwrap();
        message.set_id(11);
        message.builder_mut();
        message.set_id(12);

        // let order = message.finish();
        println!("id: {:}", message.id());

        // let mut order = message.base();
        // order.set_id(10);
        // order.set_client_id("");
        // message.finish();
        // order
    }

    #[test]
    fn builder_test() {
        build();
        let mut message = OrderMessage::new().unwrap();
        // let mut order = message.base();
        println!("id: {:}", message.id());
        // message.build(|b| {
        //     // let o = b.unsafe_mut();
        //     // b.set_id(12);
        //     process(b);
        // });
        // let m = message.finish();

        println!("done");
    }
}