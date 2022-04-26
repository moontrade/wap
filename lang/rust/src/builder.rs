/////////////////////////////////////////////////////////////////////////////
// BaseBuilder16Ref
/////////////////////////////////////////////////////////////////////////////

use std::cell::UnsafeCell;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::pin::Pin;
use std::ptr;

use crate::alloc::{Allocator, Global};
use crate::block::Block;
use crate::header::{Header, Header16, Sized16};
use crate::message;

pub struct Allocation<'s, B: Builder> {
    b: &'s mut B,
    p: *mut B::Block,
}

impl<'s, B: Builder> Allocation<'s, B> {
    fn reallocate(&mut self) {}
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

pub enum GrowResult {
    Success = 0,
    OutOfMemory = 1,
    Overflow = 2,
    Underflow = 3,
}

pub trait Builder {
    type Header: Header;
    type Block: Block;
    type Grow: Grow;
    type Allocator: Allocator;

    fn to_offset(&self, block: *mut Self::Block) -> usize;

    fn head_ptr(&self) -> *mut u8;

    fn base_ptr(&self) -> *mut u8;

    fn tail_ptr(&self) -> *mut u8;

    fn end_ptr(&self) -> *mut u8;

    fn base_size(&self) -> usize;

    fn capacity(&self) -> usize;

    fn size(&self) -> usize;

    fn grow(&mut self, by: usize) -> GrowResult;

    fn allocate(&mut self, size: usize) -> *mut Self::Block;

    fn append(&mut self, size: usize) -> *mut Self::Block;

    fn reallocate(&mut self, block: &mut Self::Block, size: usize) -> *mut Self::Block;

    fn deallocate(&mut self, block: &mut Self::Block);
}

pub trait Record {
    type Header: Header;
    type Builder: Builder;

    const SIZE: usize;

    fn new(offset: isize, builder: *mut Self::Builder) -> Self;
}

pub struct Message<
    H: Header,
    B: Builder,
    R: Record<Header=H, Builder=B>,
> {
    b: B,
    r: R,
    // _p: PhantomData<()>,
}

impl<
    H: Header,
    B: Builder,
    R: Record<Header=H, Builder=B>
> Message<H, B, R> {
    pub fn new(builder: B) -> Self {
        Self {
            b: builder,
            r: unsafe { MaybeUninit::uninit().assume_init() },
            // r: S::new(H::SIZE+2, ptr::null_mut()),
            // _p: PhantomData,
        }
    }

    pub fn builder(&mut self) -> &mut B {
        &mut self.b
    }

    pub fn record(&mut self) -> &mut R {
        self.r = R::new(H::SIZE + 2, self.builder());
        &mut self.r
    }

    pub fn build<F>(&mut self, mut f: F) where
        F: FnMut(&mut R) {
        f(self.record());
    }
}

pub struct Appender<
    H: Header,
    G: Grow = Doubled,
    A: Allocator = Global
> {
    head: *mut u8,
    base: *mut u8,
    tail: *mut u8,
    end: *mut u8,
    trash: usize,
    _phantom: PhantomData<(H, G, A)>,
}

impl<H, G, A> Appender<H, G, A>
    where
        H: Header, G: Grow, A: Allocator {
    pub fn new<R>(extra: usize) -> Option<Message<H, Self, R>>
        where
            R: Record<Header=H, Builder=Self> {
        let size = H::SIZE as usize + R::SIZE + 2;
        let capacity = size + extra;
        let p = unsafe { A::allocate(capacity) };
        if p == ptr::null_mut() {
            return None;
        }
        let builder = unsafe {
            Self {
                head: p,
                base: p.offset(H::SIZE),
                tail: p.offset(size as isize),
                end: p.offset(capacity as isize),
                trash: 0usize,
                _phantom: PhantomData,
            }
        };
        Some(Message::new(builder))
    }

    pub fn wrap(head: *mut u8, base: *mut u8, tail: *mut u8, end: *mut u8) -> Self {
        Self { head, base, tail, end, trash: 0, _phantom: PhantomData }
    }
}

impl<H: Header, G: Grow, A: Allocator> Drop for Appender<H, G, A> {
    fn drop(&mut self) {
        if self.head != ptr::null_mut() {
            A::deallocate(self.head, self.capacity());
            self.head = ptr::null_mut();
        }
    }
}

impl<H: Header, G: Grow, A: Allocator> Builder for Appender<H, G, A> {
    type Header = H;
    type Block = H::Block;
    type Grow = G;
    type Allocator = A;

    #[inline(always)]
    fn to_offset(&self, block: *mut Self::Block) -> usize {
        Self::Block::offset(self.base, block) as usize
    }

    #[inline(always)]
    fn head_ptr(&self) -> *mut u8 {
        self.head
    }

    #[inline(always)]
    fn base_ptr(&self) -> *mut u8 {
        self.base
    }

    #[inline(always)]
    fn tail_ptr(&self) -> *mut u8 {
        self.tail
    }

    #[inline(always)]
    fn end_ptr(&self) -> *mut u8 {
        self.end
    }

    #[inline(always)]
    fn base_size(&self) -> usize {
        Self::Block::get_size_value_mut(self.base)
    }

    #[inline(always)]
    fn capacity(&self) -> usize {
        unsafe { self.end as usize - self.head as usize }
    }

    #[inline(always)]
    fn size(&self) -> usize {
        H::size(self.head)
    }

    fn grow(&mut self, by: usize) -> GrowResult {
        let current_capacity = self.capacity();
        let new_capacity = G::calc(current_capacity, by);

        if new_capacity > H::Block::SIZE_LIMIT {
            return GrowResult::Overflow;
        }
        let existing_size = unsafe { self.tail as usize - self.head as usize };

        // can't shrink
        if new_capacity < current_capacity {
            return GrowResult::Underflow;
        }

        // try to reallocate
        let (new_buffer, new_buffer_capacity) = A::reallocate(self.head, new_capacity);
        if new_buffer == ptr::null_mut() {
            return GrowResult::OutOfMemory;
        }

        // in place reallocation?
        if self.head == new_buffer {
            // ensure the end is correctly set
            self.end = unsafe { new_buffer.offset(new_capacity as isize) };
            return GrowResult::Success;
        }

        self.head = new_buffer;
        self.base = unsafe { new_buffer.offset(H::SIZE) };
        self.tail = unsafe { new_buffer.offset(existing_size as isize) };
        self.end = unsafe { new_buffer.offset(new_capacity as isize) };
        GrowResult::Success
    }

    #[inline(always)]
    fn allocate(&mut self, size: usize) -> *mut Self::Block {
        self.append(size)
    }

    fn append(&mut self, size: usize) -> *mut Self::Block {
        let current_size = H::size(self.head);
        let new_size = current_size + Self::Block::OVERHEAD as usize;
        let mut new_tail = unsafe { self.tail.offset(Self::Block::OVERHEAD + size as isize) };

        if new_tail > self.end {
            match self.grow(size + Self::Block::OVERHEAD as usize) {
                GrowResult::Success => {
                    new_tail = unsafe { self.tail.offset(Self::Block::OVERHEAD + size as isize) };
                }
                _ => return ptr::null_mut(),
            }
        }

        let block = Self::Block::from_ptr(self.tail);
        self.tail = new_tail;

        // update entire message size
        H::set_size(self.head, new_size);
        // set the block's size
        block.set_size_usize(size);

        unsafe { block as *mut Self::Block }
    }

    fn reallocate(&mut self, block: &mut Self::Block, size: usize) -> *mut Self::Block {
        if block.is_free() {
            return ptr::null_mut();
        }
        let current_size = self.size();
        let offset = Self::Block::offset(self.base, block);

        // out of bounds?
        if offset < 0 || offset > current_size as isize - Self::Block::MIN_SIZE {
            return ptr::null_mut();
        }

        let current_block_size = block.size_usize();

        // Is it the last allocation?
        if current_size == offset as usize + Self::Block::OVERHEAD as usize + current_block_size {
            let new_size = current_size - current_block_size + size;

            if current_block_size >= size {
                H::set_size(self.head, new_size);
                return block;
            }

            let mut new_tail = unsafe {
                self.tail.offset(Self::Block::OVERHEAD + size as isize)
            };
            // Resize required?
            if new_tail > self.end {
                match self.grow(size + Self::Block::OVERHEAD as usize) {
                    GrowResult::Success => {
                        new_tail = unsafe { self.tail.offset(Self::Block::OVERHEAD + size as isize) };
                    }
                    _ => return ptr::null_mut(),
                }

                H::set_size(self.head, new_size);
                let block = Self::Block::from_ptr(
                    unsafe { self.base.offset(offset) }
                );
                block.set_size_usize(size);
                return block;
            } else {
                H::set_size(self.head, new_size);
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
        let block = Self::Block::from_ptr(unsafe { self.base.offset(offset) });
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
        let offset = Self::Block::offset(self.base, block);

        // out of bounds?
        if offset < 0 || offset > current_size as isize - Self::Block::MIN_SIZE {
            return;
        }

        if block.is_free() {
            return;
        }

        let block_size = block.size_usize();
        if current_size == offset as usize + Self::Block::OVERHEAD as usize + block_size {
            H::set_size(self.head, offset as usize);
            return;
        }

        self.trash += block_size + Self::Block::OVERHEAD as usize;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub trait Order {
        fn id(&self) -> u64;

        fn set_id(&mut self, id: u64) -> &mut Self;

        fn client_id(&self) -> &str;
    }

    pub trait OrderBuilder {
        // fn id(&self) -> u64;
        //
        // fn set_id(&mut self, id: u64) -> &mut Self;
        //
        // fn client_id(&self) -> &str;

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

    pub struct OrderBuilder16<
        H: Header + Header16,
        B: Builder<Header=H, Block=H::Block>> {
        builder: *mut B,
        offset: isize,
    }

    impl<H: Header + Header16, B: Builder<Header=H, Block=H::Block>> Record for OrderBuilder16<H, B> {
        type Header = H;
        type Builder = B;

        const SIZE: usize = core::mem::size_of::<Order16>() as usize;

        fn new(offset: isize, builder: *mut Self::Builder) -> Self {
            Self {
                builder,
                offset,
                // _phantom: PhantomData,
            }
        }
    }

    impl<H, B> OrderBuilder16<H, B>
        where
            H: Header + Header16,
            B: Builder<Header=H, Block=H::Block> {
        #[inline(always)]
        fn unsafe_mut(&self) -> &mut Order16 {
            unsafe { &mut *((&*self.builder).base_ptr().offset(self.offset) as *mut Order16) }
        }

        #[inline(always)]
        fn builder(&mut self) -> &mut B {
            unsafe { &mut *self.builder }
        }
    }

    impl<H, B> Order for OrderBuilder16<H, B>
        where
            H: Header + Header16,
            B: Builder<Header=H, Block=H::Block> {
        #[inline(always)]
        fn id(&self) -> u64 {
            self.unsafe_mut().id
        }

        #[inline(always)]
        fn set_id(&mut self, id: u64) -> &mut Self {
            self.unsafe_mut().id = id;
            self
        }

        #[inline(always)]
        fn client_id(&self) -> &str {
            ""
        }
    }

    impl<H, B> OrderBuilder for OrderBuilder16<H, B>
        where
            H: Header + Header16,
            B: Builder<Header=H, Block=H::Block> {
        // #[inline(always)]
        // fn id(&self) -> u64 {
        //     self.deref_mut().id
        // }
        //
        // #[inline(always)]
        // fn set_id(&mut self, id: u64) -> &mut Self {
        //     self.deref_mut().id = id;
        //     self
        // }
        //
        // #[inline(always)]
        // fn client_id(&self) -> &str {
        //     todo!()
        // }

        fn set_client_id(&mut self, value: &str) -> &mut Self {
            let block = self.builder().append(value.len());
            self
        }
    }

    pub struct OrderMessage;

    impl OrderMessage {
        fn new() -> Option<Message<
            Sized16, Appender<Sized16>,
            OrderBuilder16<Sized16, Appender<Sized16>>>> {
            Appender::<Sized16, Doubled, Global>::new::
            <OrderBuilder16<Sized16, Appender<Sized16, Doubled, Global>>>
                (32)
        }

        fn copy(src: &impl Order, dst: &mut (impl Order + OrderBuilder)) {
            dst.set_id(src.id()).
                set_client_id(src.client_id());
        }
    }

    // pub type OrderType = impl Order + OrderBuilder;

    fn process(order: &mut (impl Order + OrderBuilder)) {
        println!("order");
    }

    fn process_builder(order: &mut impl OrderBuilder) {}

    #[test]
    fn builder_test() {
        let mut o = OrderMessage::new().unwrap();
        o.record().set_id(11);
        o.build(|b| {
            let o = b.unsafe_mut();
            b.set_id(12);
            process(b);
        });
    }
}