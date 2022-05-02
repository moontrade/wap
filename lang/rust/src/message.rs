use core::cmp::max;
use core::cmp::Ordering::Greater;
use core::marker::PhantomData;
use core::mem::MaybeUninit;
use core::ops::{Deref, DerefMut};
use core::ptr;
use std::mem;

use crate::{block, builder, hash};
use crate::alloc::{Allocator, Global};
use crate::block::{Block, Block16};
use crate::builder::Builder;
use crate::hash::Hasher;
use crate::header::{Fixed16, Header, Header16};

pub trait IsFixed {}

pub trait IsFlex {}

pub trait Message {
    type Header: Header;
    type Block: Block;
    type Layout: Sized;
    type Safe: Provider;
    type Unsafe: UnsafeProvider;

    const SIZE: usize;
    const INITIAL_SIZE: usize;
    const ZERO_OFFSET: isize;
    const BASE_OFFSET: isize;
    const BASE_SIZE: isize;

    fn alloc<A: Allocator>() -> *mut u8 {
        let size = Self::INITIAL_SIZE;
        let p = A::allocate(size);
        if p == ptr::null_mut() {
            return ptr::null_mut();
        }
        Self::Header::init(p, Self::INITIAL_SIZE, Self::SIZE)
    }

    fn alloc_with_extra<A: Allocator>(extra: usize) -> (*mut u8, *mut u8) {
        let size = Self::INITIAL_SIZE + extra;
        let p = A::allocate(size);
        if p == ptr::null_mut() {
            (ptr::null_mut(), ptr::null_mut())
        } else {
            (Self::Header::init(p, Self::INITIAL_SIZE, Self::SIZE), unsafe { p.offset(size as isize) })
        }
    }
}

pub trait FlexMessage: Message {}

pub trait BuilderProvider<B: Builder>: Sized {
    fn builder_mut(&mut self) -> &mut B;

    fn root_ptr(&self) -> *mut u8;

    fn root_ptr_offset(&self, offset: isize) -> *mut u8 {
        unsafe { self.root_ptr().offset(offset) }
    }

    fn base_ptr(&self) -> *mut u8;

    fn base_offset(&self) -> usize {
        unsafe { self.base_ptr() as usize - self.root_ptr() as usize }
    }

    fn base_ptr_offset(&self, offset: isize) -> *mut u8 {
        unsafe { self.base_ptr().offset(offset) }
    }

    #[inline(always)]
    fn finish(self) -> BoxSafe<B::Message, B::Allocator>;
}

// pub trait MessageFlex<B: Builder>: Message {
//     // type Builder: Builder;
//     type BuilderRoot: BuilderProvider<B>;
//
//     fn new_builder(extra: usize) -> Option<Self::BuilderRoot>;
// }

pub trait Provider {
    fn new(base: *const u8) -> Self;

    fn base_ptr(&self) -> *const u8;
}

pub trait UnsafeProvider {
    fn new(base: *const u8, bounds: *const u8) -> Self;

    fn base_ptr(&self) -> *const u8;
}

// pub trait Box: Deref<Target=Self::Root> + DerefMut {
pub trait Box {
    type Header: Header;
    type Message: Message;
    type Allocator: Allocator;
    type Root: Sized;

    fn base_ptr(&self) -> *const u8;

    #[inline(always)]
    fn offset(&self, offset: usize) -> *const u8 {
        unsafe { self.base_ptr().offset(-Self::Header::SIZE + offset as isize) }
    }

    #[inline(always)]
    fn size(&self) -> usize {
        self.header().size()
    }

    #[inline(always)]
    fn capacity(&self) -> usize {
        self.header().size()
    }

    #[inline(always)]
    fn header(&self) -> &Self::Header {
        unsafe { &*(self.base_ptr().offset(-Self::Header::SIZE) as *mut Self::Header) }
    }

    #[inline(always)]
    fn header_mut(&mut self) -> &mut Self::Header {
        unsafe { &mut *(self.base_ptr().offset(-Self::Header::SIZE) as *mut Self::Header) }
    }
}

pub struct Inner<'a, A, B>(&'a mut A, B);

impl<'a, A, B> Deref for Inner<'a, A, B> {
    type Target = B;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.1
    }
}

pub struct InnerMut<'a, A, B>(pub &'a mut A, B);

impl<'a, A, B> Deref for InnerMut<'a, A, B> {
    type Target = B;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.1
    }
}

impl<'a, A, B> DerefMut for InnerMut<'a, A, B> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.1
    }
}

pub struct Slice<'a, B>(B, PhantomData<(&'a ())>);

impl<'a, B> Deref for Slice<'a, B> {
    type Target = B;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct SliceMut<'a, B>(B, PhantomData<(&'a ())>);

impl<'a, B> Deref for SliceMut<'a, B> {
    type Target = B;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a, B> DerefMut for SliceMut<'a, B> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////
// Safe Box
////////////////////////////////////////////////////////////////////////////////////////////////////
pub struct BoxSafe<M: Message, A: Allocator = Global> {
    root: M::Safe,
    _p: PhantomData<(M, A)>,
}

impl<R: Message, A: Allocator> BoxSafe<R, A> {
    pub fn new() -> Option<Self> {
        let p = R::alloc::<A>();
        if p == ptr::null_mut() {
            None
        } else {
            Some(Self { root: R::Safe::new(unsafe { p.offset(R::Header::SIZE) }), _p: PhantomData })
        }
    }
}

impl<R: Message, A: Allocator> BoxSafe<R, A> {
    pub(crate) fn from_header(h: *const u8) -> Self {
        Self { root: R::Safe::new(unsafe { h.offset(R::Header::SIZE) }), _p: PhantomData }
    }

    pub(crate) fn from_base(b: *const u8) -> Self {
        Self { root: R::Safe::new(b), _p: PhantomData }
    }

    pub fn into_mut(self) -> BoxSafeMut<R, A> {
        self.into()
    }
}

impl<M: Message, A: Allocator> Deref for BoxSafe<M, A> {
    type Target = M::Safe;

    fn deref(&self) -> &Self::Target {
        &self.root
    }
}

impl<M: Message, A: Allocator> Box for BoxSafe<M, A> {
    type Header = M::Header;
    type Message = M;
    type Allocator = A;
    type Root = M::Safe;

    #[inline(always)]
    fn base_ptr(&self) -> *const u8 {
        self.root.base_ptr()
    }
}

impl<R: Message, A: Allocator> Drop for BoxSafe<R, A> {
    fn drop(&mut self) {
        A::deallocate(
            unsafe { self.root.base_ptr().offset(-R::Header::SIZE) as *mut u8 },
            self.capacity(),
        )
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////
// Safe Box
////////////////////////////////////////////////////////////////////////////////////////////////////
pub struct BoxSafeMut<M: Message, A: Allocator = Global> {
    root: M::Safe,
    _p: PhantomData<(M, A)>,
}

impl<R: Message, A: Allocator> BoxSafeMut<R, A> {
    pub fn new() -> Option<Self> {
        let p = R::alloc::<A>();
        if p == ptr::null_mut() {
            None
        } else {
            Some(Self { root: R::Safe::new(unsafe { p.offset(R::Header::SIZE) }), _p: PhantomData })
        }
    }
}

impl<R: Message, A: Allocator> BoxSafeMut<R, A> {
    pub(crate) fn from_header(h: *const u8) -> Self {
        Self { root: R::Safe::new(unsafe { h.offset(R::Header::SIZE) }), _p: PhantomData }
    }

    pub(crate) fn from_base(b: *const u8) -> Self {
        Self { root: R::Safe::new(b), _p: PhantomData }
    }

    pub fn into(self) -> BoxSafe<R, A> {
        let b = BoxSafe::from_base(self.base_ptr());
        mem::forget(self);
        b
    }

    pub fn finish(self) -> BoxSafe<R, A> {
        let b = BoxSafe::from_base(self.base_ptr());
        mem::forget(self);
        b
    }
}

impl<M: Message, A: Allocator> Deref for BoxSafeMut<M, A> {
    type Target = M::Safe;

    fn deref(&self) -> &Self::Target {
        &self.root
    }
}

impl<M: Message, A: Allocator> DerefMut for BoxSafeMut<M, A> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.root
    }
}

impl<M: Message, A: Allocator> Box for BoxSafeMut<M, A> {
    type Header = M::Header;
    type Message = M;
    type Allocator = A;
    type Root = M::Safe;

    #[inline(always)]
    fn base_ptr(&self) -> *const u8 {
        self.root.base_ptr()
    }
}

impl<R: Message, A: Allocator> Drop for BoxSafeMut<R, A> {
    fn drop(&mut self) {
        A::deallocate(
            unsafe { self.base_ptr().offset(-R::Header::SIZE) as *mut u8 },
            self.capacity(),
        )
    }
}

impl<R: Message, A: Allocator> Into<BoxSafe<R, A>> for BoxSafeMut<R, A> {
    fn into(self) -> BoxSafe<R, A> {
        let into = BoxSafe::from_base(self.base_ptr());
        mem::forget(self);
        into
    }
}

impl<R: Message, A: Allocator> From<BoxSafe<R, A>> for BoxSafeMut<R, A> {
    fn from(s: BoxSafe<R, A>) -> Self {
        let to = Self { root: R::Safe::new(s.base_ptr()), _p: PhantomData };
        mem::forget(s);
        to
    }
}


////////////////////////////////////////////////////////////////////////////////////////////////////
// Unsafe
////////////////////////////////////////////////////////////////////////////////////////////////////
pub struct BoxUnsafe<M: Message, A: Allocator = Global> {
    root: M::Unsafe,
    _p: PhantomData<(M, A)>,
}

impl<M: Message + IsFixed, A: Allocator> BoxUnsafe<M, A> {}

impl<M: Message, A: Allocator> Deref for BoxUnsafe<M, A> {
    type Target = M::Unsafe;

    fn deref(&self) -> &Self::Target {
        &self.root
    }
}

impl<M: Message, A: Allocator> DerefMut for BoxUnsafe<M, A> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.root
    }
}

impl<M: Message, A: Allocator> Box for BoxUnsafe<M, A> {
    type Header = M::Header;
    type Message = M;
    type Allocator = A;
    type Root = M::Unsafe;

    #[inline(always)]
    fn base_ptr(&self) -> *const u8 {
        self.root.base_ptr()
    }
}

impl<M: Message, A: Allocator> Drop for BoxUnsafe<M, A> {
    fn drop(&mut self) {
        A::deallocate(unsafe { self.root.base_ptr().offset(-M::Header::SIZE) as *mut u8 },
                      self.capacity())
    }
}


////////////////////////////////////////////////////////////////////////////////////////////////////
// Safe Box
////////////////////////////////////////////////////////////////////////////////////////////////////
pub struct BoxUnsafeMut<M: Message, A: Allocator = Global> {
    root: M::Unsafe,
    _p: PhantomData<(M, A)>,
}

impl<R: Message, A: Allocator> BoxUnsafeMut<R, A> {}

impl<R: Message, A: Allocator> BoxUnsafeMut<R, A> {
    // pub(crate) fn from_header(h: *const u8) -> Self {
    //     Self { root: R::Safe::new(unsafe { h.offset(R::Header::SIZE) }), _p: PhantomData }
    // }
    //
    // pub(crate) fn from_base(b: *const u8) -> Self {
    //     Self { root: R::Safe::new(b), _p: PhantomData }
    // }
    //
    // pub fn into(self) -> Box<R, A> {
    //     let b = Box::from_base(self.base_ptr());
    //     mem::forget(self);
    //     b
    // }
    //
    // pub fn finish(self) -> Box<R, A> {
    //     let b = Box::from_base(self.base_ptr());
    //     mem::forget(self);
    //     b
    // }
}

impl<M: Message, A: Allocator> Deref for BoxUnsafeMut<M, A> {
    type Target = M::Unsafe;

    fn deref(&self) -> &Self::Target {
        &self.root
    }
}

impl<M: Message, A: Allocator> DerefMut for BoxUnsafeMut<M, A> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.root
    }
}

impl<M: Message, A: Allocator> Box for BoxUnsafeMut<M, A> {
    type Header = M::Header;
    type Message = M;
    type Allocator = A;
    type Root = M::Unsafe;

    #[inline(always)]
    fn base_ptr(&self) -> *const u8 {
        self.root.base_ptr()
    }
}

impl<R: Message, A: Allocator> Drop for BoxUnsafeMut<R, A> {
    fn drop(&mut self) {
        A::deallocate(
            unsafe { self.base_ptr().offset(-R::Header::SIZE) as *mut u8 },
            self.capacity(),
        )
    }
}

// impl<R: Message, A: Allocator> Into<Box<R, A>> for BoxUnsafeMut<R, A> {
//     fn into(self) -> Box<R, A> {
//         let into = Box::from_base(self.base_ptr());
//         mem::forget(self);
//         into
//     }
// }
//
// impl<R: Message, A: Allocator> From<Box<R, A>> for BoxUnsafeMut<R, A> {
//     fn from(s: BoxUnsafe<R, A>) -> Self {
//         let to = Self { root: R::Unsafe::new(s.root.base_ptr(), s.root.), _p: PhantomData };
//         mem::forget(s);
//         to
//     }
// }

#[cfg(test)]
mod tests {
    use core::mem;
    use core::ops::Deref;
    use core::ptr::slice_from_raw_parts;

    use crate::block::Block16;
    use crate::builder::{Appender, Builder};
    use crate::hash::{hash, hash_default, hash_sized};
    use crate::header::{Flex16, Header16};

    use super::*;

    pub trait Order {
        type Price: Price;

        // Read
        fn id(&self) -> u64;
        fn price(&self) -> Slice<Self::Price>;

        // Stable Write - underlying buffer must never reallocate
        fn set_id(&mut self, value: u64) -> &mut Self;
        fn price_mut(&mut self) -> SliceMut<Self::Price>;
    }

    pub trait OrderBuilder: Order {
        type PriceBuilder: Price;

        // Unstable Write - underlying buffer may reallocate
        fn set_client_id(&mut self, value: &str) -> &mut Self;
    }

    pub trait Price {
        fn open(&self) -> f64;
        fn high(&self) -> f64;
        fn low(&self) -> f64;
        fn close(&self) -> f64;

        fn set_open(&mut self, value: f64) -> &mut Self;
        fn set_high(&mut self, value: f64) -> &mut Self;
        fn set_low(&mut self, value: f64) -> &mut Self;
        fn set_close(&mut self, value: f64) -> &mut Self;
    }

    /////////////////////////////////////////////////////////////////////////////
    // Order - Layout
    /////////////////////////////////////////////////////////////////////////////
    #[repr(C, packed)]
    pub struct OrderData {
        id: u64,
        client_id: [u8; 10],
        price: PriceData,
    }

    impl OrderData {
        pub fn id(&self) -> u64 {
            u64::from_le(self.id)
        }

        pub fn price(&self) -> &PriceData {
            &self.price
        }


        pub fn set_id(&mut self, value: u64) -> &mut Self {
            self.id = value.to_le();
            self
        }

        pub fn price_mut(&mut self) -> &mut PriceData {
            &mut self.price
        }
    }

    //////////////////////////////////////////////////////////////////////////////
    // OrderMessage
    //////////////////////////////////////////////////////////////////////////////

    pub struct OrderMessage<H: Header = Flex16>(PhantomData<H>);

    impl<H: Header> Message for OrderMessage<H> {
        type Header = H;
        type Block = H::Block;
        type Layout = OrderData;
        type Safe = OrderSafe<Self>;
        type Unsafe = OrderUnsafe<Self>;

        const SIZE: usize = mem::size_of::<Self::Layout>();
        const INITIAL_SIZE: usize = H::SIZE as usize + Self::SIZE;
        const ZERO_OFFSET: isize = H::SIZE - H::Block::SIZE_BYTES;
        const BASE_OFFSET: isize = H::SIZE;
        const BASE_SIZE: isize = Self::SIZE as isize;
    }

    impl<H: Header> FlexMessage for OrderMessage<H> {}

    impl OrderMessage<Flex16> {
        pub fn new() -> Option<BoxSafeMut<OrderMessage<Flex16>>> {
            BoxSafeMut::new()
        }

        fn new_builder(extra: usize) -> Option<OrderBuilderRoot<Appender<Self>>> {
            if let Some(a) = Appender::<Self>::new(extra) {
                Some(OrderBuilderRoot::<Appender<Self>>(a, PhantomData))
            } else {
                None
            }
        }
    }

    impl<H: Header + Header16> OrderMessage<H> {
        pub fn new_with_header() -> Option<BoxSafe<OrderMessage<H>>> {
            BoxSafe::new()
        }

        fn new_builder_with_header(extra: usize) -> Option<OrderBuilderRoot<Appender<Self>>> {
            if let Some(a) = Appender::<Self>::new(extra) {
                Some(OrderBuilderRoot::<Appender<Self>>(a, PhantomData))
            } else {
                None
            }
        }
    }

    pub struct OrderBuilderRoot<B: Builder>(B, PhantomData<(B)>);

    impl<B: Builder> BuilderProvider<B> for OrderBuilderRoot<B> {
        fn builder_mut(&mut self) -> &mut B {
            &mut self.0
        }

        fn root_ptr(&self) -> *mut u8 {
            self.0.root_ptr()
        }

        fn base_ptr(&self) -> *mut u8 {
            unsafe { self.0.offset(B::Message::BASE_OFFSET) }
        }

        fn finish(self) -> BoxSafe<B::Message, B::Allocator> {
            self.0.finish()
        }
    }

    impl<B: Builder> OrderBuilderRoot<B> {
        fn layout_ptr(&self) -> &OrderData {
            unsafe { &*((self.0).offset(B::Message::BASE_OFFSET) as *const OrderData) }
        }

        fn layout_ptr_mut(&mut self) -> &mut OrderData {
            unsafe { &mut *((self.0).offset(B::Message::BASE_OFFSET) as *const OrderData as *mut OrderData) }
        }
    }

    impl<B: Builder> Order for OrderBuilderRoot<B> {
        type Price = PriceBuilderNested<B>;

        fn id(&self) -> u64 {
            self.layout_ptr().id()
        }

        fn price(&self) -> Slice<Self::Price> {
            Slice(PriceBuilderNested::<B>(
                &self.0, 18, PhantomData), PhantomData)
        }

        fn set_id(&mut self, value: u64) -> &mut Self {
            self.layout_ptr_mut().set_id(value);
            self
        }

        fn price_mut(&mut self) -> SliceMut<Self::Price> {
            SliceMut(PriceBuilderNested::<B>(
                &self.0, 18, PhantomData), PhantomData)
        }
    }

    impl<B: Builder> OrderBuilder for OrderBuilderRoot<B> {
        type PriceBuilder = PriceBuilderNested<B>;

        fn set_client_id(&mut self, value: &str) -> &mut Self {
            self
        }
    }

    pub struct OrderBuilderNested<B: Builder>(*const B, isize, PhantomData<(B)>);

    impl<B: Builder> OrderBuilderNested<B> {
        #[inline(always)]
        fn builder_ptr(&self) -> &B {
            unsafe { &*self.0 }
        }

        #[inline(always)]
        fn layout_ptr(&self) -> &OrderData {
            unsafe { &*(self.builder_ptr().offset(self.1) as *const OrderData) }
        }

        #[inline(always)]
        fn layout_ptr_mut(&mut self) -> &mut OrderData {
            unsafe { &mut *(self.builder_ptr().offset(self.1) as *mut OrderData) }
        }
    }

    impl<B: Builder> Order for OrderBuilderNested<B> {
        type Price = PriceBuilderNested<B>;

        fn id(&self) -> u64 {
            self.layout_ptr().id()
        }

        fn price(&self) -> Slice<Self::Price> {
            Slice(PriceBuilderNested::<B>(
                self.0, self.1 + 18, PhantomData),
                  PhantomData)
        }

        fn set_id(&mut self, value: u64) -> &mut Self {
            self.layout_ptr_mut().set_id(value);
            self
        }

        fn price_mut(&mut self) -> SliceMut<Self::Price> {
            SliceMut(PriceBuilderNested::<B>(
                self.0, self.1 + 18, PhantomData),
                     PhantomData)
        }
    }


    pub struct PriceBuilderNested<B: Builder>(*const B, isize, PhantomData<(B)>);

    impl<B: Builder> PriceBuilderNested<B> {
        #[inline(always)]
        fn builder_ptr(&self) -> &B {
            unsafe { &*self.0 }
        }

        #[inline(always)]
        fn layout_ptr(&self) -> &PriceData {
            unsafe { &*(self.builder_ptr().offset(self.1) as *const PriceData) }
        }

        #[inline(always)]
        fn layout_ptr_mut(&mut self) -> &mut PriceData {
            unsafe { &mut *(self.builder_ptr().offset(self.1) as *mut PriceData) }
        }
    }

    impl<B: Builder> Price for PriceBuilderNested<B> {
        fn open(&self) -> f64 {
            self.layout_ptr().open()
        }

        fn high(&self) -> f64 {
            self.layout_ptr().high()
        }

        fn low(&self) -> f64 {
            self.layout_ptr().low()
        }

        fn close(&self) -> f64 {
            self.layout_ptr().close()
        }

        fn set_open(&mut self, value: f64) -> &mut Self {
            self.layout_ptr_mut().set_open(value);
            self
        }

        fn set_high(&mut self, value: f64) -> &mut Self {
            self.layout_ptr_mut().set_high(value);
            self
        }

        fn set_low(&mut self, value: f64) -> &mut Self {
            self.layout_ptr_mut().set_low(value);
            self
        }

        fn set_close(&mut self, value: f64) -> &mut Self {
            self.layout_ptr_mut().set_close(value);
            self
        }
    }

    ////////////////////////////////////////////////////////////////////////////////////////////////
    // OrderSafe
    ////////////////////////////////////////////////////////////////////////////////////////////////

    pub struct OrderSafe<R: Message> {
        base: *const OrderData,
        _p: PhantomData<(R)>,
    }

    impl<R: Message> Provider for OrderSafe<R> {
        fn new(base: *const u8) -> Self {
            Self { base: unsafe { base as *const OrderData }, _p: PhantomData }
        }

        fn base_ptr(&self) -> *const u8 {
            unsafe { self.base as *const u8 }
        }
    }

    impl<R: Message> Order for OrderSafe<R> {
        type Price = impl Price;

        fn id(&self) -> u64 {
            unsafe { (&*(self.base)).id }
        }

        fn set_id(&mut self, value: u64) -> &mut Self {
            unsafe { (&mut *(self.base as *mut OrderData)).id = value; }
            self
        }

        fn price(&self) -> Slice<Self::Price> {
            Slice(PriceSafe::<R>::new(
                unsafe { (self.base as *const u8).offset(10) }),
                  PhantomData)
        }

        fn price_mut(&mut self) -> SliceMut<Self::Price> {
            SliceMut(PriceSafe::<R>::new(
                unsafe { (self.base as *const u8).offset(10) }),
                     PhantomData)
        }
    }

    ////////////////////////////////////////////////////////////////////////////////////////////////
    // OrderUnsafe
    ////////////////////////////////////////////////////////////////////////////////////////////////
    pub struct OrderUnsafe<R: Message> {
        base: *const OrderData,
        bounds: *const u8,
        _p: PhantomData<(R)>,
    }

    impl<R: Message> UnsafeProvider for OrderUnsafe<R> {
        fn new(base: *const u8, bounds: *const u8) -> Self {
            Self { base: unsafe { base as *const OrderData }, bounds, _p: PhantomData }
        }

        fn base_ptr(&self) -> *const u8 {
            unsafe { self.base as *const u8 }
        }
    }

    pub struct OrderNested<R: Message> {
        base: *const OrderData,
        root: *const u8,
        _p: PhantomData<(R)>,
    }

    pub struct OrderUnsafeNested<R: Message> {
        base: *const OrderData,
        bounds: *const u8,
        root: *const u8,
        _p: PhantomData<(R)>,
    }

    impl<R: Message> OrderNested<R> {
        fn new(base: *const u8, root: *const u8) -> Self {
            Self { base: unsafe { base as *const OrderData }, root, _p: PhantomData }
        }
    }

    impl<R: Message> OrderUnsafeNested<R> {
        fn new(base: *const u8, bounds: *const u8, root: *const u8) -> Self {
            Self { base: unsafe { base as *const OrderData }, bounds, root, _p: PhantomData }
        }
    }

    pub struct PriceMessage<H: Header>(PhantomData<H>);

    impl<H: Header> Message for PriceMessage<H> {
        type Header = H;
        type Block = H::Block;
        type Layout = PriceData;
        type Safe = PriceSafe<Self>;
        type Unsafe = PriceUnsafe<Self>;

        const SIZE: usize = mem::size_of::<Self::Layout>();
        const INITIAL_SIZE: usize = H::SIZE as usize + Self::SIZE;
        const ZERO_OFFSET: isize = H::SIZE - H::Block::SIZE_BYTES;
        const BASE_OFFSET: isize = H::SIZE;
        const BASE_SIZE: isize = Self::SIZE as isize;
    }

    impl<H: Header> IsFixed for PriceMessage<H> {}

    impl PriceMessage<Fixed16> {
        pub fn new() -> Option<BoxSafe<PriceMessage<Fixed16>>> {
            BoxSafe::new()
        }
    }

    pub struct PriceSafe<R: Message>(*const PriceData, PhantomData<R>);

    impl<R: Message> Provider for PriceSafe<R> {
        fn new(base: *const u8) -> Self {
            Self(unsafe { base as *const PriceData }, PhantomData)
        }

        fn base_ptr(&self) -> *const u8 {
            unsafe { self.0 as *const u8 }
        }
    }

    impl<R: Message> Price for PriceSafe<R> {
        #[inline(always)]
        fn open(&self) -> f64 {
            unsafe { (&*self.0).open() }
        }

        #[inline(always)]
        fn high(&self) -> f64 {
            unsafe { (&*self.0).high() }
        }

        #[inline(always)]
        fn low(&self) -> f64 {
            unsafe { (&*self.0).low() }
        }

        #[inline(always)]
        fn close(&self) -> f64 {
            unsafe { (&*self.0).close() }
        }

        #[inline(always)]
        fn set_open(&mut self, value: f64) -> &mut Self {
            unsafe { (&mut *(self.0 as *mut PriceData)).set_open(value); }
            self
        }

        #[inline(always)]
        fn set_high(&mut self, value: f64) -> &mut Self {
            unsafe { (&mut *(self.0 as *mut PriceData)).set_open(value); }
            self
        }

        #[inline(always)]
        fn set_low(&mut self, value: f64) -> &mut Self {
            unsafe { (&mut *(self.0 as *mut PriceData)).set_low(value); }
            self
        }

        #[inline(always)]
        fn set_close(&mut self, value: f64) -> &mut Self {
            unsafe { (&mut *(self.0 as *mut PriceData)).set_close(value); }
            self
        }
    }

    pub struct PriceUnsafe<R: Message>(*const u8, *const u8, PhantomData<R>);

    impl<R: Message> UnsafeProvider for PriceUnsafe<R> {
        fn new(base: *const u8, bounds: *const u8) -> Self {
            Self(base, bounds, PhantomData)
        }

        fn base_ptr(&self) -> *const u8 {
            unsafe { self.0 as *const u8 }
        }
    }

    impl<M: Message> Price for PriceUnsafe<M> {
        #[inline(always)]
        fn open(&self) -> f64 {
            unsafe {
                if self.0.offset(8) as usize > self.1 as usize { 0f64 } else { (&*(self.0 as *const PriceData)).open() }
            }
        }

        #[inline(always)]
        fn high(&self) -> f64 {
            unsafe {
                if self.0.offset(16) as usize > self.1 as usize { 0f64 } else { (&*(self.0 as *const PriceData)).close() }
            }
        }

        #[inline(always)]
        fn low(&self) -> f64 {
            unsafe {
                if self.0.offset(24) as usize > self.1 as usize { 0f64 } else { (&*(self.0 as *const PriceData)).low() }
            }
        }

        #[inline(always)]
        fn close(&self) -> f64 {
            unsafe {
                if self.0.offset(32) as usize > self.1 as usize { 0f64 } else { (&*(self.0 as *const PriceData)).close() }
            }
        }

        #[inline(always)]
        fn set_open(&mut self, value: f64) -> &mut Self {
            unsafe {
                if self.0.offset(8) as usize > self.1 as usize { self } else {
                    (&mut *(self.0 as *mut PriceData)).set_open(value);
                    self
                }
            }
        }

        #[inline(always)]
        fn set_high(&mut self, value: f64) -> &mut Self {
            unsafe {
                if self.0.offset(16) as usize > self.1 as usize { self } else {
                    (&mut *(self.0 as *mut PriceData)).set_high(value);
                    self
                }
            }
        }

        #[inline(always)]
        fn set_low(&mut self, value: f64) -> &mut Self {
            unsafe {
                if self.0.offset(24) as usize > self.1 as usize { self } else {
                    (&mut *(self.0 as *mut PriceData)).set_low(value);
                    self
                }
            }
        }

        #[inline(always)]
        fn set_close(&mut self, value: f64) -> &mut Self {
            unsafe {
                if self.0.offset(32) as usize > self.1 as usize { self } else {
                    (&mut *(self.0 as *mut PriceData)).set_close(value);
                    self
                }
            }
        }
    }

    // #[inline(always)]
    // fn copy_price<'a, 'b>(from: &impl Price, to: &mut impl Price) {
    //     to.set_open(from.open())
    //         .set_high(from.high())
    //         .set_low(from.low())
    //         .set_close(from.close());
    // }

    #[derive(Copy, Clone, Debug)]
    #[repr(C, packed)]
    pub struct PriceData {
        open: f64,
        high: f64,
        low: f64,
        close: f64,
    }

    impl PriceData {
        pub fn new(open: f64, high: f64, low: f64, close: f64) -> Self {
            Self { open, high, low, close }
        }
    }

    impl<'a> Price for PriceData {
        #[inline(always)]
        fn open(&self) -> f64 {
            f64::from_bits(f64::to_bits(self.open).to_le())
        }
        #[inline(always)]
        fn high(&self) -> f64 {
            f64::from_bits(f64::to_bits(self.high).to_le())
        }
        #[inline(always)]
        fn low(&self) -> f64 {
            f64::from_bits(f64::to_bits(self.low).to_le())
        }
        #[inline(always)]
        fn close(&self) -> f64 {
            f64::from_bits(f64::to_bits(self.close).to_le())
        }

        #[inline(always)]
        fn set_open(&mut self, value: f64) -> &mut Self {
            self.open = f64::from_bits(f64::to_bits(value).to_le());
            self
        }

        #[inline(always)]
        fn set_high(&mut self, value: f64) -> &mut Self {
            self.close = f64::from_bits(f64::to_bits(value).to_le());
            self
        }

        #[inline(always)]
        fn set_low(&mut self, value: f64) -> &mut Self {
            self.low = f64::from_bits(f64::to_bits(value).to_le());
            self
        }

        #[inline(always)]
        fn set_close(&mut self, value: f64) -> &mut Self {
            self.close = f64::from_bits(f64::to_bits(value).to_le());
            self
        }
    }

    fn process_price(price: &mut impl Price) {
        price.set_open(4198.25);
        println!("open: {:}", price.open());
    }

    fn build_order2(o: &mut impl Order) {
        o.price_mut().close();
        o.price_mut().set_close(113.34);
    }

    fn build_order() -> impl Box<Message=OrderMessage> {
        // let mut o = OrderMessage::new().unwrap();
        let mut o = OrderMessage::new_builder(0).unwrap();

        // build_order2(o.deref_mut());
        build_order2(&mut o);

        let mut price = o.price_mut();
        price.set_open(133.34);
        price.set_low(11.33);
        o.price_mut();

        o.price_mut().set_close(34.44);
        o.price().open();

        let o = o.finish();

        println!("open: {:}", o.price().open());
        o
    }

    #[test]
    fn root_message() {
        build_order();
        let mut o = OrderMessage::new().unwrap();
        println!("size: {:}", o.size());
        println!("capacity: {:}", o.capacity());

        // let (mut p, mut o) = o.price_mut();
        // p.set_high(11.0);
        // o.price().high();
        // o.price_mut().0.set_open(10.0);
    }

    fn build() {
        let mut p = PriceMessage::new().unwrap();
        // let mut pp = PriceMessage::<Fixed16>::new_safe();
        // pp.set_open(11.0);
        // pp.base_ptr();
    }

    // fn return_price<'a>(o: &'a impl Order<'a>) -> impl Price {
    //     o.price()
    // }

    #[test]
    fn message() {
        build();
        println!("done");
    }
}