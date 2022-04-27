/////////////////////////////////////////////////////////////////////////////
// BaseBuilder16Ref
/////////////////////////////////////////////////////////////////////////////

use std::borrow::BorrowMut;
use std::cell::UnsafeCell;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::pin::Pin;
use std::ptr;

use crate::alloc::{Allocator, Doubled, Global, Grow, GrowResult};
use crate::block::Block;
use crate::header::{Header, Header16, Fixed16};
use crate::message;

pub trait Builder {
    type Header: Header;
    type Block: Block;
    type Grow: Grow;
    type Allocator: Allocator;

    fn to_vptr(&self, block: *mut Self::Block) -> usize;

    fn head_ptr(&self) -> *mut u8;

    fn base_ptr(&self) -> *mut u8;

    fn tail_ptr(&self) -> *mut u8;

    fn end_ptr(&self) -> *mut u8;

    fn base_size(&self) -> usize;

    fn capacity(&self) -> usize;

    fn size(&self) -> usize;

    fn take(&mut self) -> (*const u8, *const u8, *const u8, *const u8);

    fn grow(&mut self, by: usize) -> GrowResult;

    // fn allocate0(&mut self, size: usize) -> Allocation<'a, Self> where Self: Sized;

    fn allocate(&mut self, size: usize) -> *mut Self::Block;

    fn append(&mut self, size: usize) -> *mut Self::Block;

    fn reallocate(&mut self, block: &mut Self::Block, size: usize) -> *mut Self::Block;

    fn deallocate(&mut self, block: &mut Self::Block);
}

pub trait Record {
    type Header: Header;
    type Builder: Builder;
    type Completed: message::Record;

    const SIZE: usize;

    fn new(offset: isize, builder: *mut Self::Builder) -> Self;
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

pub struct Message<
    H: Header,
    B: Builder,
    C: message::Record<Header = H>,
    R: Record<Header=H, Builder=B, Completed=C>,
> {
    b: B,
    r: R,
    // _phantom: PhantomData<(&'a ())>,
}

impl<
    H: Header,
    B: Builder,
    C: message::Record<Header = H>,
    R: Record<Header=H, Builder=B, Completed=C>,
> Message<H, B, C, R> {
    #[inline(always)]
    pub fn new(builder: B) -> Self {
        Self {
            b: builder,
            r: unsafe { MaybeUninit::uninit().assume_init() },
            // r: S::new(H::SIZE+2, ptr::null_mut()),
            // _phantom: PhantomData,
        }
    }

    #[inline(always)]
    pub fn builder(&mut self) -> &mut B {
        // unsafe { &mut *(&mut self.b as *mut B) }
        &mut self.b
    }

    #[inline(always)]
    pub fn record(&mut self) -> &mut R {
        self.r = R::new(H::SIZE + 2, self.builder());
        &mut self.r
        // unsafe { &mut *(&mut self.r as *mut R) }
    }

    pub fn build<F>(&mut self, mut f: F) where
        F: FnMut(&mut R) {
        f(self.record());
    }

    pub fn finish(mut self) -> message::Flex<H, C, B::Allocator> {
        let (head, base, tail, end) = self.b.take();
        message::Flex::from_builder(head, base, tail, end)
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
    pub fn new<C, R>(extra: usize) -> Option<Message<H, Self, C, R>>
        where
            C: message::Record<Header=H>,
            R: Record<Header=H, Builder=Self, Completed=C> {
        let size = H::SIZE as usize + R::SIZE + 2;
        let capacity = size + extra;
        let p = unsafe { A::allocate(capacity) };
        if p == ptr::null_mut() {
            return None;
        }
        let m = Message::new(unsafe {
            Self {
                head: p,
                base: p.offset(H::SIZE),
                tail: p.offset(size as isize),
                end: p.offset(capacity as isize),
                trash: 0usize,
                _phantom: PhantomData,
            }
        });
        Some(m)
    }

    pub fn wrap(head: *mut u8, base: *mut u8, tail: *mut u8, end: *mut u8) -> Self {
        Self { head, base, tail, end, trash: 0, _phantom: PhantomData }
    }
}

impl<H, G, A> Drop for Appender<H, G, A>
    where
        H: Header,
        G: Grow,
        A: Allocator {
    fn drop(&mut self) {
        if self.head != ptr::null_mut() {
            A::deallocate(self.head, self.capacity());
            self.head = ptr::null_mut();
        }
    }
}

impl<H, G, A> Builder for Appender<H, G, A>
    where
        H: Header, G: Grow, A: Allocator {
    type Header = H;
    type Block = H::Block;
    type Grow = G;
    type Allocator = A;

    #[inline(always)]
    fn to_vptr(&self, block: *mut Self::Block) -> usize {
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
        H::raw_size(self.head)
    }

    fn take(&mut self) -> (*const u8, *const u8, *const u8, *const u8) {
        let head = self.head;
        self.head = ptr::null_mut();
        (head, self.base, self.tail, self.end)
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

    // fn allocate0(&mut self, size: usize) -> Allocation<'a, Self> {
    //     Allocation::new(unsafe { &mut *(self as *mut Self) }, self.allocate(size))
    // }

    #[inline(always)]
    fn allocate(&mut self, size: usize) -> *mut Self::Block {
        self.append(size)
    }

    fn append(&mut self, size: usize) -> *mut Self::Block {
        let current_size = H::raw_size(self.head);
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
        H::raw_set_size(self.head, new_size);
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
                H::raw_set_size(self.head, new_size);
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

                H::raw_set_size(self.head, new_size);
                let block = Self::Block::from_ptr(
                    unsafe { self.base.offset(offset) }
                );
                block.set_size_usize(size);
                return block;
            } else {
                H::raw_set_size(self.head, new_size);
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
            H::raw_set_size(self.head, offset as usize);
            return;
        }

        self.trash += block_size + Self::Block::OVERHEAD as usize;
    }
}

#[cfg(test)]
mod tests {
    use std::process::Command;
    use crate::message::Base;
    use super::*;

    pub trait Order {
        fn id(&self) -> u64;

        fn set_id(&mut self, id: u64) -> &mut Self;

        fn client_id(&self) -> &str;
    }

    pub trait OrderBuilder {
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

    pub struct OrderSafe<H: Header + Header16 = Fixed16> {
        p: *const Order16,
        base: message::Base<H>,
    }

    impl<H: Header + Header16> message::Record for OrderSafe<H> {
        type Header = H;
        type Layout = Order16;

        const SIZE: usize = core::mem::size_of::<Order16>();

        fn new(p: *const u8, _: *const u8, m: Base<Self::Header>) -> Self {
            Self { p: unsafe { p as *const Order16 }, base: m }
        }
    }

    impl<H: Header + Header16> core::ops::Deref for OrderSafe<H> {
        type Target = Order16;

        fn deref(&self) -> &Self::Target {
            unsafe { &*self.p }
        }
    }

    impl<H: Header + Header16> core::ops::DerefMut for OrderSafe<H> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            unsafe { &mut *(self.p as *mut Self::Target) }
        }
    }

    pub struct OrderBuilder16<
        H: Header + Header16,
        B: Builder<Header=H, Block=H::Block>,
        C: message::Record<Header=H>> {
        builder: *mut B,
        offset: isize,
        _phantom: PhantomData<C>,
    }

    impl<H, B, C> Record for OrderBuilder16<H, B, C>
        where
            H: Header + Header16,
            B: Builder<Header=H, Block=H::Block>,
            C: message::Record<Header=H> {
        type Header = H;
        type Builder = B;
        type Completed = C;

        const SIZE: usize = core::mem::size_of::<Order16>() as usize;

        fn new(offset: isize, builder: *mut Self::Builder) -> Self {
            Self {
                builder,
                offset,
                _phantom: PhantomData,
            }
        }
    }

    impl<H, B, C> OrderBuilder16<H, B, C>
        where
            H: Header + Header16,
            B: Builder<Header=H, Block=H::Block>,
            C: message::Record<Header=H> {
        #[inline(always)]
        fn unsafe_mut(&self) -> &mut Order16 {
            unsafe { &mut *((&*self.builder).base_ptr().offset(self.offset) as *mut Order16) }
        }

        #[inline(always)]
        fn builder(&mut self) -> &mut B {
            unsafe { &mut *self.builder }
        }
    }

    impl<H, B, C> Order for OrderBuilder16<H, B, C>
        where
            H: Header + Header16,
            B: Builder<Header=H, Block=H::Block>,
            C: message::Record<Header=H> {
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

    impl<H, B, C> OrderBuilder for OrderBuilder16<H, B, C>
        where
            H: Header + Header16,
            B: Builder<Header=H, Block=H::Block>,
            C: message::Record<Header=H> {
        #[inline(always)]
        fn set_client_id(&mut self, value: &str) -> &mut Self {
            let block = self.builder().append(value.len());
            self
        }
    }

    pub struct OrderMessage;

    impl OrderMessage {
        fn new() -> Option<Message<
            Fixed16, Appender<Fixed16>,
            OrderSafe<Fixed16>,
            OrderBuilder16<Fixed16, Appender<Fixed16>, OrderSafe<Fixed16>>>> {

            let m = Appender::<Fixed16, Doubled, Global>::new::
            <OrderSafe<Fixed16>, OrderBuilder16<Fixed16, Appender<Fixed16, Doubled, Global>, OrderSafe<Fixed16>>>
                (32);
            m
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

    struct C;

    struct A {
        c: C,
        // b: B<'a>,
        // _phantom: PhantomData<(&'a ())>,
    }

    // impl A {
    //     fn new<'a>() -> A<'a> {
    //         Self { _phantom: PhantomData }
    //     }
    // }

    impl A {
        fn new() -> A {
            Self {
                c: C {},
                // _phantom: PhantomData
            }
        }

        fn b(&mut self) -> B {
            B::new(&mut self.c)
        }
    }

    struct B<'a> {
        a: &'a mut C,
    }

    impl<'a> B<'a> {
        fn new(a: &'a mut C) -> Self {
            Self { a }
        }
    }

    #[test]
    fn lifetime() {
        let mut a = A::new();
        let mut b = a.b();
        let mut b = B::new(&mut a.c);
        // let mut b = a.b();
        // let mut c = a;
    }

    #[test]
    fn builder_test() {
        let mut o = OrderMessage::new().unwrap();
        o.record().set_id(11);
        o.build(|b| {
            // let o = b.unsafe_mut();
            // b.set_id(12);
            process(b);
        });
        let m = o.finish();

        println!("done");
    }
}