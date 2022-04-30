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
//     type Header: 'a + Header;
// }
//
// pub trait WithLayout<'a> {
//     type Layout: Sized;
// }
//
// pub trait WithRecord<'a> {
//     type Record: Message<'a>;
// }

pub trait Message<'a> {
    type Header: Header;
    type Layout: Sized;
    type Safe: Provider<'a>;
    type Unsafe: UnsafeProvider<'a>;

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

pub trait Provider<'a> {
    fn new(base: *const u8) -> Self;

    fn base_ptr(&self) -> *const u8;
}

pub trait UnsafeProvider<'a> {
    fn new(base: *const u8, bounds: *const u8) -> Self;

    fn base_ptr(&self) -> *const u8;
}

pub trait Box<'a>: Deref<Target=Self::Root> + DerefMut {
    type Header: Header;
    type Message: Message<'a>;
    type Allocator: Allocator;
    type Root: Sized;

    fn offset(&self, offset: usize) -> *const u8;

    fn size(&self) -> usize;

    fn capacity(&self) -> usize;

    fn header(&self) -> &Self::Header;

    fn header_mut(&mut self) -> &mut Self::Header;
}

////////////////////////////////////////////////////////////////////////////////////////////////////
// Safe Box
////////////////////////////////////////////////////////////////////////////////////////////////////
pub struct Safe<'a, M: 'a + Message<'a>, A: Allocator = Global> {
    root: M::Safe,
    _p: PhantomData<(&'a (), M, A)>,
}

impl<'a, R: 'a + Message<'a>, A: Allocator> Safe<'a, R, A> {
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
}

impl<'a, M: 'a + Message<'a>, A: Allocator> Deref for Safe<'a, M, A> {
    type Target = M::Safe;

    fn deref(&self) -> &Self::Target {
        &self.root
    }
}

impl<'a, M: 'a + Message<'a>, A: Allocator> DerefMut for Safe<'a, M, A> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.root
    }
}

impl<'a, M: 'a + Message<'a>, A: Allocator> Box<'a> for Safe<'a, M, A> {
    type Header = M::Header;
    type Message = M;
    type Allocator = A;
    type Root = M::Safe;

    fn offset(&self, offset: usize) -> *const u8 {
        unsafe { self.base_ptr().offset(-M::Header::SIZE + offset as isize) }
    }

    fn size(&self) -> usize {
        self.header().size()
    }

    fn capacity(&self) -> usize {
        self.header().size()
    }

    fn header(&self) -> &M::Header {
        unsafe { &*(self.base_ptr().offset(-M::Header::SIZE) as *mut M::Header) }
    }

    fn header_mut(&mut self) -> &mut M::Header {
        unsafe { &mut *(self.base_ptr().offset(-M::Header::SIZE) as *mut M::Header) }
    }
}

impl<'a, R: 'a + Message<'a>, A: Allocator> Drop for Safe<'a, R, A> {
    fn drop(&mut self) {
        let h = unsafe { self.base_ptr().offset(-R::Header::SIZE) as *mut u8 };
        A::deallocate(h, self.capacity())
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////
// Unsafe - Capacity is fixed based on a const core::alloc::Layout
////////////////////////////////////////////////////////////////////////////////////////////////////
pub struct Unsafe<'a, M: 'a + Message<'a>, A: Allocator = Global> {
    root: M::Unsafe,
    _p: PhantomData<(&'a (), M, A)>,
}

impl<'a, M: 'a + Message<'a>, A: Allocator> Unsafe<'a, M, A> {
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

impl<'a, M: 'a + Message<'a>, A: Allocator> Deref for Unsafe<'a, M, A> {
    type Target = M::Unsafe;

    fn deref(&self) -> &Self::Target {
        &self.root
    }
}

impl<'a, M: 'a + Message<'a>, A: Allocator> DerefMut for Unsafe<'a, M, A> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.root
    }
}

impl<'a, M: 'a + Message<'a>, A: Allocator> Box<'a> for Unsafe<'a, M, A> {
    type Header = M::Header;
    type Message = M;
    type Allocator = A;
    type Root = M::Unsafe;

    fn offset(&self, offset: usize) -> *const u8 {
        unsafe { self.base_ptr().offset(-M::Header::SIZE + offset as isize) }
    }

    fn size(&self) -> usize {
        self.header().size()
    }

    fn capacity(&self) -> usize {
        self.header().size()
    }

    fn header(&self) -> &M::Header {
        unsafe { &*(self.base_ptr().offset(-M::Header::SIZE) as *mut M::Header) }
    }

    fn header_mut(&mut self) -> &mut M::Header {
        unsafe { &mut *(self.base_ptr().offset(-M::Header::SIZE) as *mut M::Header) }
    }
}

impl<'a, M: 'a + Message<'a>, A: Allocator> Drop for Unsafe<'a, M, A> {
    fn drop(&mut self) {
        let h = unsafe { self.base_ptr().offset(-M::Header::SIZE) as *mut u8 };
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

    pub trait Order<'a> {
        type Price: Price<'a>;

        fn id(&self) -> u64;

        fn set_id(&mut self, value: u64) -> &mut Self;

        fn price(&'a self) -> Self::Price;

        fn price_mut(&mut self) -> (Self::Price, &mut Self);

        fn set_price<'b>(&'a mut self, price: impl Price<'b>) -> &'a mut Self;
    }

    pub trait OrderBuilder<'a>: Order<'a> {
        fn set_client_id(&mut self, value: &str) -> &mut Self;
    }

    pub trait Price<'a> {
        fn wap_hash(&self) -> u64;

        fn copy_to<'b>(&self, to: &mut impl Price<'b>);

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
        price: PriceData,
    }

    //////////////////////////////////////////////////////////////////////////////
    // OrderMessage
    //////////////////////////////////////////////////////////////////////////////

    pub struct OrderMessage<'a, H: 'a + Header = Flex16>(PhantomData<(&'a (), H)>);

    impl<'a, H: 'a + Header> Message<'a> for OrderMessage<'a, H> {
        type Header = H;
        type Layout = OrderData;
        type Safe = OrderProvider<'a, Self>;
        type Unsafe = OrderUnsafeProvider<'a, Self>;

        const SIZE: usize = mem::size_of::<Self::Layout>();
        const INITIAL_SIZE: usize = H::SIZE as usize + Self::SIZE;
        const ZERO_OFFSET: isize = H::SIZE - H::Block::SIZE_BYTES;
        const BASE_OFFSET: isize = H::SIZE;
        const BASE_SIZE: isize = Self::SIZE as isize;
    }

    impl<'a> OrderMessage<'a, Flex16> {
        pub fn new() -> Option<Safe<'a, OrderMessage<'a, Flex16>>> {
            Safe::new()
        }

        pub fn new_unsafe() -> Option<Unsafe<'a, OrderMessage<'a, Flex16>>> {
            Unsafe::new()
        }
    }

    impl<'a, H: Header + Header16> OrderMessage<'a, H> {
        pub fn new_with_header() -> Option<Safe<'a, OrderMessage<'a, H>>> {
            Safe::new()
        }

        pub fn new_unsafe_with_header() -> Option<Unsafe<'a, OrderMessage<'a, H>>> {
            Unsafe::new()
        }
    }

    ////////////////////////////////////////////////////////////////////////////////////////////////
    // OrderSafe
    ////////////////////////////////////////////////////////////////////////////////////////////////

    pub struct OrderProvider<'a, R: 'a + Message<'a>> {
        base: *const OrderData,
        _p: PhantomData<(&'a (), R)>,
    }

    impl<'a, R: 'a + Message<'a>> Provider<'a> for OrderProvider<'a, R> {
        fn new(base: *const u8) -> Self {
            Self { base: unsafe { base as *const OrderData }, _p: PhantomData }
        }

        fn base_ptr(&self) -> *const u8 {
            unsafe { self.base as *const u8 }
        }
    }

    impl<'a, R: 'a + Message<'a>> Order<'a> for OrderProvider<'a, R> {
        type Price = PriceProvider<'a, R>;

        fn id(&self) -> u64 {
            unsafe { (&*(self.base)).id }
        }

        fn set_id(&mut self, value: u64) -> &mut Self {
            unsafe { (&mut *(self.base as *mut OrderData)).id = value; }
            self
        }

        fn price(&'a self) -> Self::Price {
            Self::Price::new(unsafe { (self.base as *const u8).offset(10) })
        }

        fn price_mut(&mut self) -> (Self::Price, &mut Self) {
            (Self::Price::new(unsafe { (self.base as *const u8).offset(10) }), self)
        }

        fn set_price<'b>(&'a mut self, price: impl Price<'b>) -> &'a mut Self {
            self
        }
    }

    ////////////////////////////////////////////////////////////////////////////////////////////////
    // OrderUnsafe
    ////////////////////////////////////////////////////////////////////////////////////////////////
    pub struct OrderUnsafeProvider<'a, R: 'a + Message<'a>> {
        base: *const OrderData,
        bounds: *const u8,
        _p: PhantomData<(&'a (), R)>,
    }

    impl<'a, R: 'a + Message<'a>> UnsafeProvider<'a> for OrderUnsafeProvider<'a, R> {
        fn new(base: *const u8, bounds: *const u8) -> Self {
            Self { base: unsafe { base as *const OrderData }, bounds, _p: PhantomData }
        }

        fn base_ptr(&self) -> *const u8 {
            unsafe { self.base as *const u8 }
        }
    }


    pub struct OrderNestedProvider<'a, R: 'a + Message<'a>> {
        base: *const OrderData,
        root: *const u8,
        _p: PhantomData<(&'a (), R)>,
    }

    pub struct OrderUnsafeNestedProvider<'a, R: 'a + Message<'a>> {
        base: *const OrderData,
        bounds: *const u8,
        root: *const u8,
        _p: PhantomData<(&'a (), R)>,
    }

    impl<'a, R: 'a + Message<'a>> OrderNestedProvider<'a, R> {
        fn new(base: *const u8, root: *const u8) -> Self {
            Self { base: unsafe { base as *const OrderData }, root, _p: PhantomData }
        }
    }

    impl<'a, R: 'a + Message<'a>> OrderUnsafeNestedProvider<'a, R> {
        fn new(base: *const u8, bounds: *const u8, root: *const u8) -> Self {
            Self { base: unsafe { base as *const OrderData }, bounds, root, _p: PhantomData }
        }
    }


    pub struct PriceMessage<'a, H: 'a + Header>(PhantomData<(&'a (), H)>);

    impl<'a, H: 'a + Header> Message<'a> for PriceMessage<'a, H> {
        type Header = H;
        type Layout = PriceData;
        type Safe = PriceProvider<'a, Self>;
        type Unsafe = PriceUnsafeProvider<'a, Self>;

        const SIZE: usize = mem::size_of::<Self::Layout>();
        const INITIAL_SIZE: usize = H::SIZE as usize + Self::SIZE;
        const ZERO_OFFSET: isize = H::SIZE - H::Block::SIZE_BYTES;
        const BASE_OFFSET: isize = H::SIZE;
        const BASE_SIZE: isize = Self::SIZE as isize;
    }

    impl<'a> PriceMessage<'a, Fixed16> {
        pub fn new() -> Option<Safe<'a, PriceMessage<'a, Fixed16>>> {
            Safe::new()
        }
    }

    pub struct PriceProvider<'a, R: 'a + Message<'a>> {
        base: *const PriceData,
        _p: PhantomData<(&'a (), R)>,
    }

    impl<'a, R: 'a + Message<'a>> Provider<'a> for PriceProvider<'a, R> {
        fn new(base: *const u8) -> Self {
            Self { base: unsafe { base as *const PriceData }, _p: PhantomData }
        }

        fn base_ptr(&self) -> *const u8 {
            unsafe { self.base as *const u8 }
        }
    }

    impl<'a, R: 'a + Message<'a>> Price<'a> for PriceProvider<'a, R> {
        #[inline(always)]
        fn wap_hash(&self) -> u64 {
            hash_default(unsafe { self.base as *const u8 },
                         mem::size_of::<PriceData>() as u64)
        }

        #[inline(always)]
        fn copy_to<'b>(&self, to: &mut impl Price<'b>) {
            copy_price(self, to);
        }

        #[inline(always)]
        fn open(&self) -> f64 {
            unsafe { (&*self.base).open() }
        }

        #[inline(always)]
        fn high(&self) -> f64 {
            unsafe { (&*self.base).high() }
        }

        #[inline(always)]
        fn low(&self) -> f64 {
            unsafe { (&*self.base).low() }
        }

        #[inline(always)]
        fn close(&self) -> f64 {
            unsafe { ((&*self.base)).close() }
        }

        #[inline(always)]
        fn set_open(&mut self, value: f64) -> &mut Self {
            unsafe { (&mut *(self.base as *mut PriceData)).set_open(value); }
            self
        }

        #[inline(always)]
        fn set_high(&mut self, value: f64) -> &mut Self {
            unsafe { (&mut *(self.base as *mut PriceData)).set_open(value); }
            self
        }

        #[inline(always)]
        fn set_low(&mut self, value: f64) -> &mut Self {
            unsafe { (&mut *(self.base as *mut PriceData)).set_low(value); }
            self
        }

        #[inline(always)]
        fn set_close(&mut self, value: f64) -> &mut Self {
            unsafe { (&mut *(self.base as *mut PriceData)).set_close(value); }
            self
        }
    }


    pub struct PriceUnsafeProvider<'a, R: 'a + Message<'a>> {
        base: *const PriceData,
        bounds: *const u8,
        _p: PhantomData<(&'a (), R)>,
    }

    impl<'a, R: 'a + Message<'a>> UnsafeProvider<'a> for PriceUnsafeProvider<'a, R> {
        fn new(base: *const u8, bounds: *const u8) -> Self {
            Self { base: unsafe { base as *const PriceData }, bounds, _p: PhantomData }
        }

        fn base_ptr(&self) -> *const u8 {
            unsafe { self.base as *const u8 }
        }
    }

    impl<'a, M: 'a + Message<'a>> Price<'a> for PriceUnsafeProvider<'a, M> {
        #[inline(always)]
        fn wap_hash(&self) -> u64 {
            hash_default(unsafe { self.base as *const u8 },
                         mem::size_of::<PriceData>() as u64)
        }

        #[inline(always)]
        fn copy_to<'b>(&self, to: &mut impl Price<'b>) {
            copy_price(self, to);
        }

        #[inline(always)]
        fn open(&self) -> f64 {
            unsafe {
                if self.bounds.offset(8) as usize > self.bounds as usize { 0f64 } else { (&*self.base).open() }
            }
        }

        #[inline(always)]
        fn high(&self) -> f64 {
            unsafe {
                if self.bounds.offset(16) as usize > self.bounds as usize { 0f64 } else { (&*self.base).close() }
            }
        }

        #[inline(always)]
        fn low(&self) -> f64 {
            unsafe {
                if self.bounds.offset(24) as usize > self.bounds as usize { 0f64 } else { (&*self.base).low() }
            }
        }

        #[inline(always)]
        fn close(&self) -> f64 {
            unsafe {
                if self.bounds.offset(32) as usize > self.bounds as usize { 0f64 } else { (&*self.base).close() }
            }
        }

        #[inline(always)]
        fn set_open(&mut self, value: f64) -> &mut Self {
            unsafe {
                if self.bounds.offset(8) as usize > self.bounds as usize { self } else {
                    (&mut *(self.base as *mut PriceData)).set_open(value);
                    self
                }
            }
        }

        #[inline(always)]
        fn set_high(&mut self, value: f64) -> &mut Self {
            unsafe {
                if self.bounds.offset(16) as usize > self.bounds as usize { self } else {
                    (&mut *(self.base as *mut PriceData)).set_high(value);
                    self
                }
            }
        }

        #[inline(always)]
        fn set_low(&mut self, value: f64) -> &mut Self {
            unsafe {
                if self.bounds.offset(24) as usize > self.bounds as usize { self } else {
                    (&mut *(self.base as *mut PriceData)).set_low(value);
                    self
                }
            }
        }

        #[inline(always)]
        fn set_close(&mut self, value: f64) -> &mut Self {
            unsafe {
                if self.bounds.offset(32) as usize > self.bounds as usize { self } else {
                    (&mut *(self.base as *mut PriceData)).set_close(value);
                    self
                }
            }
        }
    }



    pub struct PriceBuilderProvider<'a, M: 'a + Message<'a>, B: Builder> {
        builder: *const B,
        offset: isize,
        _p: PhantomData<(&'a (), M)>,
    }

    impl<'a, R: 'a + Message<'a>, B: Builder> PriceBuilderProvider<'a, R, B> {
        fn new(base: *const u8, offset: isize, builder: *const B) -> Self {
            Self { builder, offset, _p: PhantomData }
        }

        fn deref(&self) -> &PriceData {
            unsafe { &*((&*self.builder).offset(self.offset) as *const PriceData) }
        }

        fn deref_mut(&mut self) -> &mut PriceData {
            unsafe { &mut *((&*self.builder).offset(self.offset) as *mut PriceData) }
        }
    }

    impl<'a, R: 'a + Message<'a>, B: Builder> Price<'a> for PriceBuilderProvider<'a, R, B> {
        #[inline(always)]
        fn wap_hash(&self) -> u64 {
            hash_default(unsafe { self.builder.offset(self.offset) as *const u8 },
                         mem::size_of::<PriceData>() as u64)
        }

        #[inline(always)]
        fn copy_to<'b>(&self, to: &mut impl Price<'b>) {
            copy_price(self, to);
        }

        #[inline(always)]
        fn open(&self) -> f64 {
            self.deref().open()
        }

        #[inline(always)]
        fn high(&self) -> f64 {
            self.deref().high()
        }

        #[inline(always)]
        fn low(&self) -> f64 {
            self.deref().low()
        }

        #[inline(always)]
        fn close(&self) -> f64 {
            self.deref().close()
        }

        #[inline(always)]
        fn set_open(&mut self, value: f64) -> &mut Self {
            self.deref_mut().set_open(value);
            self
        }

        #[inline(always)]
        fn set_high(&mut self, value: f64) -> &mut Self {
            self.deref_mut().set_high(value);
            self
        }

        #[inline(always)]
        fn set_low(&mut self, value: f64) -> &mut Self {
            self.deref_mut().set_low(value);
            self
        }

        #[inline(always)]
        fn set_close(&mut self, value: f64) -> &mut Self {
            self.deref_mut().set_close(value);
            self
        }
    }


    #[inline(always)]
    fn copy_price<'a, 'b>(from: &impl Price<'a>, to: &mut impl Price<'b>) {
        to.set_open(from.open())
            .set_high(from.high())
            .set_low(from.low())
            .set_close(from.close());
    }

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

    impl<'a> Price<'a> for PriceData {
        #[inline(always)]
        fn wap_hash(&self) -> u64 {
            hash_default(unsafe { self as *const Self as *const u8 },
                         mem::size_of::<PriceData>() as u64)
        }

        #[inline(always)]
        fn copy_to<'b>(&self, to: &mut impl Price<'b>) {
            copy_price(self, to);
        }

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


    fn process_order2<'a>(order: &mut impl Order<'a>) {
        order.set_id(100);
        println!("order");
    }

    fn process_price<'a>(price: &mut impl Price<'a>) {
        price.set_open(4198.25);
        println!("open: {:}", price.open());
    }

    #[test]
    fn root_message() {
        let mut o = OrderMessage::new().unwrap();
        let mut ou = OrderMessage::new_unsafe().unwrap();
        println!("size: {:}", o.size());
        println!("capacity: {:}", o.capacity());

        let (mut p, mut o) = o.price_mut();
        p.set_high(11.0);
        o.price().high();
    }

    fn build() {
        let mut p = PriceMessage::new().unwrap();
        p.base;
    }

    fn return_price<'a>(o: &'a impl Order<'a>) -> impl Price<'a> {
        o.price()
    }

    #[test]
    fn message() {
        println!("done");
    }
}