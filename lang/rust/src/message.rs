use core::alloc::Layout;
use core::cmp::max;
use core::cmp::Ordering::Greater;
use core::marker::PhantomData;
use core::mem::MaybeUninit;
use core::ops::{Deref, DerefMut};
use core::ptr;

use crate::{block, builder, hash};
use crate::alloc::{Allocator, Global};
use crate::block::{Block, Block16};
use crate::hash::Hasher;
use crate::header::{Fixed16, Header, Header16};

pub trait IsFixed {}

pub trait IsFlex {}

// pub trait WithAllocator<'a> {
//     type Allocator: Allocator;
// }
//
// pub trait WithHeader<'a> {
//     type Header: Header;
// }
//
// pub trait WithLayout<'a> {
//     type Layout: Sized;
// }
//
// pub trait WithRecord<'a> {
//     type Record: Message;
// }

pub trait Message {
    type Header: Header;
    type Layout: Sized;
    type Safe: Provider;
    type Unsafe: UnsafeProvider;

    const SIZE: usize;
    const INITIAL_SIZE: usize;
    const ZERO_OFFSET: isize;
    const BASE_OFFSET: isize;
    const BASE_SIZE: isize;
}

pub trait RootPointer {
    fn root_ptr(&self) -> *const u8;
}

pub trait BasePointer {
    fn base_ptr(&self) -> *const u8;
}

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

    fn offset(&self, offset: usize) -> *const u8;

    fn size(&self) -> usize;

    fn capacity(&self) -> usize;

    fn header(&self) -> &Self::Header;

    fn header_mut(&mut self) -> &mut Self::Header;
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
pub struct Safe<M: Message, A: Allocator = Global> {
    root: M::Safe,
    _p: PhantomData<(M, A)>,
}

impl<R: Message, A: Allocator> Safe<R, A> {
    pub fn new() -> Option<Self> {
        let size = R::INITIAL_SIZE;
        let p = A::allocate(size);
        if p == ptr::null_mut() {
            return None;
        }
        let header = unsafe { (&mut *(p as *mut R::Header)) };
        header.set_size(size).set_base_size(R::SIZE);

        Some(Self { root: R::Safe::new(unsafe { p.offset(R::Header::SIZE) }), _p: PhantomData })
    }

    pub fn base(&self) -> &R::Safe {
        &self.root
    }

    pub fn base_mut0(&mut self) -> InnerMut<Self, R::Safe> {
        InnerMut(self, R::Safe::new(self.root.base_ptr()))
    }

    pub fn base_mut(&mut self) -> SliceMut<R::Safe> {
        SliceMut(R::Safe::new(self.root.base_ptr()), PhantomData)
    }

    // pub fn base_mut(&mut self) -> R::Safe {
    //     R::Safe::new(self.root.base_ptr())
    // }
}

// impl<'a, M: Message, A: Allocator> Deref for Safe<'a, M, A> {
//     type Target = M::Safe;
//
//     fn deref(&self) -> &Self::Target {
//         &self.root
//     }
// }
//
// impl<'a, M: Message, A: Allocator> DerefMut for Safe<'a, M, A> {
//     fn deref_mut(&mut self) -> &mut Self::Target {
//         &mut self.root
//     }
// }

impl<M: Message, A: Allocator> Box for Safe<M, A> {
    type Header = M::Header;
    type Message = M;
    type Allocator = A;
    type Root = M::Safe;

    fn offset(&self, offset: usize) -> *const u8 {
        unsafe { self.root.base_ptr().offset(-M::Header::SIZE + offset as isize) }
    }

    fn size(&self) -> usize {
        self.header().size()
    }

    fn capacity(&self) -> usize {
        self.header().size()
    }

    fn header(&self) -> &M::Header {
        unsafe { &*(self.root.base_ptr().offset(-M::Header::SIZE) as *mut M::Header) }
    }

    fn header_mut(&mut self) -> &mut M::Header {
        unsafe { &mut *(self.root.base_ptr().offset(-M::Header::SIZE) as *mut M::Header) }
    }
}

impl<R: Message, A: Allocator> Drop for Safe<R, A> {
    fn drop(&mut self) {
        let h = unsafe { self.root.base_ptr().offset(-R::Header::SIZE) as *mut u8 };
        A::deallocate(h, self.capacity())
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////
// Unsafe - Capacity is fixed based on a const core::alloc::Layout
////////////////////////////////////////////////////////////////////////////////////////////////////
pub struct Unsafe<M: Message, A: Allocator = Global> {
    root: M::Unsafe,
    _p: PhantomData<(M, A)>,
}

impl<M: Message, A: Allocator> Unsafe<M, A> {
    pub fn new() -> Option<Self> {
        let size = M::INITIAL_SIZE;
        let p = A::allocate(size);
        if p == ptr::null_mut() {
            return None;
        }
        let header = unsafe { (&mut *(p as *mut M::Header)) };
        header.set_size(size).set_base_size(M::SIZE);

        Some(Self {
            root: M::Unsafe::new(unsafe { p.offset(M::Header::SIZE) },
                                 unsafe { p.offset(M::Header::SIZE + M::BASE_SIZE) }),
            _p: PhantomData,
        })
    }
}

impl<M: Message, A: Allocator> Deref for Unsafe<M, A> {
    type Target = M::Unsafe;

    fn deref(&self) -> &Self::Target {
        &self.root
    }
}

impl<M: Message, A: Allocator> DerefMut for Unsafe<M, A> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.root
    }
}

impl<M: Message, A: Allocator> Box for Unsafe<M, A> {
    type Header = M::Header;
    type Message = M;
    type Allocator = A;
    type Root = M::Unsafe;

    fn offset(&self, offset: usize) -> *const u8 {
        unsafe { self.root.base_ptr().offset(-M::Header::SIZE + offset as isize) }
    }

    fn size(&self) -> usize {
        self.header().size()
    }

    fn capacity(&self) -> usize {
        self.header().size()
    }

    fn header(&self) -> &M::Header {
        unsafe { &*(self.root.base_ptr().offset(-M::Header::SIZE) as *mut M::Header) }
    }

    fn header_mut(&mut self) -> &mut M::Header {
        unsafe { &mut *(self.root.base_ptr().offset(-M::Header::SIZE) as *mut M::Header) }
    }
}

impl<M: Message, A: Allocator> Drop for Unsafe<M, A> {
    fn drop(&mut self) {
        let h = unsafe { self.root.base_ptr().offset(-M::Header::SIZE) as *mut u8 };
        A::deallocate(h, self.capacity())
    }
}

#[cfg(test)]
mod tests {
    use core::mem;
    use core::ops::Deref;
    use core::ptr::slice_from_raw_parts;

    use crate::block::Block16;
    use crate::builder::Builder;
    use crate::hash::{hash, hash_default, hash_sized};
    use crate::header::{Flex16, Header16};

    use super::*;

    pub trait Order {
        type Price: Price;

        fn id(&self) -> u64;

        fn set_id(&mut self, value: u64) -> &mut Self;

        fn price(&self) -> Slice<Self::Price>;

        fn price_mut(&mut self) -> SliceMut<Self::Price>;
    }

    pub trait OrderBuilder: Order {
        fn set_client_id(&mut self, value: &str) -> &mut Self;
    }

    pub trait Price {
        // fn wap_hash(&self) -> u64;

        // fn copy_to<'b>(&self, to: &mut impl Price);

        fn open(&self) -> f64;
        fn high(&self) -> f64;
        fn low(&self) -> f64;
        fn close(&self) -> f64;

        fn set_open(&mut self, value: f64) -> &mut Self;
        fn set_high(&mut self, value: f64) -> &mut Self;
        fn set_low(&mut self, value: f64) -> &mut Self;
        fn set_close(&mut self, value: f64) -> &mut Self;
    }


    // pub trait PriceMutable: Price {
    //
    // }


    /////////////////////////////////////////////////////////////////////////////
    // Order - Layout
    /////////////////////////////////////////////////////////////////////////////
    #[repr(C, packed)]
    pub struct OrderData {
        id: u64,
        client_id: [u8; 10],
        price: PriceData,
    }

    //////////////////////////////////////////////////////////////////////////////
    // OrderMessage
    //////////////////////////////////////////////////////////////////////////////

    pub struct OrderMessage<H: Header = Flex16>(PhantomData<H>);

    impl<H: Header> Message for OrderMessage<H> {
        type Header = H;
        type Layout = OrderData;
        type Safe = OrderProvider<Self>;
        type Unsafe = OrderUnsafe<Self>;

        const SIZE: usize = mem::size_of::<Self::Layout>();
        const INITIAL_SIZE: usize = H::SIZE as usize + Self::SIZE;
        const ZERO_OFFSET: isize = H::SIZE - H::Block::SIZE_BYTES;
        const BASE_OFFSET: isize = H::SIZE;
        const BASE_SIZE: isize = Self::SIZE as isize;
    }

    impl OrderMessage<Flex16> {
        pub fn new() -> Option<Safe<OrderMessage<Flex16>>> {
            Safe::new()
        }

        pub fn new_unsafe() -> Option<Unsafe<OrderMessage<Flex16>>> {
            Unsafe::new()
        }
    }

    impl<H: Header + Header16> OrderMessage<H> {
        pub fn new_with_header() -> Option<Safe<OrderMessage<H>>> {
            Safe::new()
        }

        pub fn new_unsafe_with_header() -> Option<Unsafe<OrderMessage<H>>> {
            Unsafe::new()
        }
    }

    ////////////////////////////////////////////////////////////////////////////////////////////////
    // OrderSafe
    ////////////////////////////////////////////////////////////////////////////////////////////////

    pub struct OrderProvider<R: Message> {
        base: *const OrderData,
        _p: PhantomData<(R)>,
    }

    impl<R: Message> Provider for OrderProvider<R> {
        fn new(base: *const u8) -> Self {
            Self { base: unsafe { base as *const OrderData }, _p: PhantomData }
        }

        fn base_ptr(&self) -> *const u8 {
            unsafe { self.base as *const u8 }
        }
    }

    impl<R: Message> Order for OrderProvider<R> {
        type Price = impl Price;

        fn id(&self) -> u64 {
            unsafe { (&*(self.base)).id }
        }

        fn set_id(&mut self, value: u64) -> &mut Self {
            unsafe { (&mut *(self.base as *mut OrderData)).id = value; }
            self
        }

        fn price(&self) -> Slice<Self::Price> {
            Slice(PriceSafe::<R>::new(unsafe { (self.base as *const u8).offset(10) }), PhantomData)
        }

        fn price_mut(&mut self) -> SliceMut<Self::Price> {
            SliceMut(PriceSafe::<R>::new(unsafe { (self.base as *const u8).offset(10) }), PhantomData)
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
        type Layout = PriceData;
        type Safe = PriceSafe<Self>;
        type Unsafe = PriceUnsafe<Self>;

        const SIZE: usize = mem::size_of::<Self::Layout>();
        const INITIAL_SIZE: usize = H::SIZE as usize + Self::SIZE;
        const ZERO_OFFSET: isize = H::SIZE - H::Block::SIZE_BYTES;
        const BASE_OFFSET: isize = H::SIZE;
        const BASE_SIZE: isize = Self::SIZE as isize;
    }

    impl PriceMessage<Fixed16> {
        pub fn new() -> Option<Safe<PriceMessage<Fixed16>>> {
            Safe::new()
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

    // pub struct PriceBuilderProvider<'a, M: Message, B: Builder> {
    //     builder: &'a mut B,
    //     offset: isize,
    //     _p: PhantomData<(M)>,
    // }
    //
    // impl<'a, R: Message, B: Builder> PriceBuilderProvider<R, B> {
    //     fn new(base: *const u8, offset: isize, builder: &'a mut B) -> Self {
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
    // impl<R: Message, B: Builder> Price for PriceBuilderProvider<R, B> {
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

    // impl<'a, R: Message, B: Builder> PriceMutable for PriceBuilderProvider<'a, R, B> {
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

    fn build_order() {
        let mut o = OrderMessage::new().unwrap();

        let mut order = o.base_mut();
        build_order2(order.deref_mut());

        let mut price = order.price_mut();
        price.set_open(133.34);
        price.set_low(11.33);
        o.base_mut().price_mut().set_close(34.44);

        let mut p = o.base().price();
        p.open();
    }

    #[test]
    fn root_message() {
        build_order();
        let mut o = OrderMessage::new().unwrap();
        let mut ou = OrderMessage::new_unsafe().unwrap();
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