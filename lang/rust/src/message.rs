use core::marker::PhantomData;
use core::ptr;
use std::borrow::BorrowMut;
use std::cell::UnsafeCell;
use std::cmp::max;
use std::cmp::Ordering::Greater;
use std::ops::{Deref, DerefMut};

use crate::{block, builder, hash};
use crate::alloc::{Allocator, Global};
use crate::block::{Block, Block16};
use crate::hash::Hasher;
use crate::header::{Fixed16, Header, Header16};

pub trait Fixed {}

pub trait Message {
    type Header: Header;
    type Record: Record;
    type Allocator: Allocator;

    /// Size of entire message including header.
    fn size(&self) -> usize;

    /// Capacity of the underlying buffer.
    fn capacity(&self) -> usize;

    /// Reference to the header.
    fn header(&self) -> &Self::Header;

    /// Mutable reference to the header.
    fn header_mut(&mut self) -> &mut Self::Header;

    /// Reference to the base record.
    fn base(&self) -> &Self::Record;

    /// Mutable reference to the base record.
    fn base_mut(&mut self) -> &mut Self::Record;
}

pub struct FixedSafe<
    H: Header,
    R: Fixed + Record<Header=H> + Deref + DerefMut,
    A: Allocator = Global,
> {
    head: FixedHead<H>,
    base: R,
    _p: PhantomData<(H, R, A)>,
}

impl<H, R, A> FixedSafe<H, R, A>
    where H: Header,
          R: Fixed + Record<Header=H> + Deref + DerefMut,
          A: Allocator {
    const INITIAL_SIZE: usize = H::SIZE as usize + R::SIZE;
    const LAYOUT: core::alloc::Layout = unsafe { core::alloc::Layout::from_size_align_unchecked(Self::INITIAL_SIZE, 1) };

    fn new() -> Option<Self> {
        let size = Self::INITIAL_SIZE;
        let p = A::allocate(size);
        if p == ptr::null_mut() {
            return None;
        }
        let base = unsafe { p.offset(H::SIZE) };
        let base_end = unsafe { base.offset(R::SIZE as isize) };
        Some(Self {
            head: FixedHead::new_with_size(p, size),
            base: R::new(base, base_end, Base::new(p)),
            _p: PhantomData,
        })
    }
}

impl<H, R, A> Message for FixedSafe<H, R, A>
    where H: Header,
          R: Fixed + Record<Header=H> + Deref + DerefMut,
          A: Allocator {
    type Header = H;
    type Record = R;
    type Allocator = A;

    #[inline(always)]
    fn size(&self) -> usize {
        self.header().size()
    }

    #[inline(always)]
    fn capacity(&self) -> usize {
        self.size()
    }

    #[inline(always)]
    fn header(&self) -> &Self::Header {
        unsafe { &*self.head.head }
    }

    #[inline(always)]
    fn header_mut(&mut self) -> &mut Self::Header {
        unsafe { &mut *(self.head.head as *mut Self::Header) }
    }

    #[inline(always)]
    fn base(&self) -> &Self::Record {
        &self.base
    }

    #[inline(always)]
    fn base_mut(&mut self) -> &mut Self::Record {
        &mut self.base
    }
}

/////////////////////////////////////////////////////////////////////////////
// FixedSafe - Drop
/////////////////////////////////////////////////////////////////////////////

impl<H, R, A> Drop for FixedSafe<H, R, A>
    where H: Header,
          R: Fixed + Record<Header=H> + Deref + DerefMut,
          A: Allocator {
    fn drop(&mut self) {
        if self.head.head != ptr::null_mut() {
            A::deallocate_layout(self.head.head as *mut u8, Self::LAYOUT);
        }
    }
}

/////////////////////////////////////////////////////////////////////////////
// FixedSafe - Deref
/////////////////////////////////////////////////////////////////////////////

impl<H, R, A> Deref for FixedSafe<H, R, A>
    where H: Header,
          R: Fixed + Record<Header=H> + Deref + DerefMut,
          A: Allocator {
    type Target = R::Target;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

/////////////////////////////////////////////////////////////////////////////
// FixedSafe - DerefMut
/////////////////////////////////////////////////////////////////////////////

impl<H, R, A> DerefMut for FixedSafe<H, R, A>
    where H: Header,
          R: Fixed + Record<Header=H> + Deref + DerefMut,
          A: Allocator {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

pub struct FixedUnsafe<
    H: Header,
    R: Fixed + Record<Header=H>,
    A: Allocator = Global,
> {
    head: FixedHead<H>,
    base: R,
    _p: PhantomData<(H, R, A)>,
}

impl<H, R, A> FixedUnsafe<H, R, A>
    where H: Header,
          R: Fixed + Record<Header=H>,
          A: Allocator {
    const INITIAL_SIZE: usize = H::SIZE as usize + R::SIZE + 2;
    const LAYOUT: core::alloc::Layout = unsafe { core::alloc::Layout::from_size_align_unchecked(Self::INITIAL_SIZE, 1) };

    fn new() -> Option<Self> {
        let size = Self::INITIAL_SIZE;
        let p = A::allocate(size);
        if p == ptr::null_mut() {
            return None;
        }
        let base = unsafe { p.offset(H::SIZE) };
        let base_end = unsafe { base.offset(R::SIZE as isize) };
        Some(Self {
            head: FixedHead::new_with_size(p, size),
            base: R::new(base, base_end, Base::new(p)),
            _p: PhantomData,
        })
    }
}

impl<H, R, A> Message for FixedUnsafe<H, R, A>
    where H: Header,
          R: Fixed + Record<Header=H>,
          A: Allocator {
    type Header = H;
    type Record = R;
    type Allocator = A;

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
        unsafe { &*self.head.head }
    }

    #[inline(always)]
    fn header_mut(&mut self) -> &mut Self::Header {
        unsafe { &mut *(self.head.head as *mut Self::Header) }
    }

    #[inline(always)]
    fn base(&self) -> &Self::Record {
        &self.base
    }

    #[inline(always)]
    fn base_mut(&mut self) -> &mut Self::Record {
        &mut self.base
    }
}

/////////////////////////////////////////////////////////////////////////////
// FixedSafe - Drop
/////////////////////////////////////////////////////////////////////////////

impl<H, R, A> Drop for FixedUnsafe<H, R, A>
    where H: Header,
          R: Fixed + Record<Header=H>,
          A: Allocator {
    fn drop(&mut self) {
        if self.head.head != ptr::null_mut() {
            A::deallocate_layout(self.head.head as *mut u8, Self::LAYOUT);
        }
    }
}

/////////////////////////////////////////////////////////////////////////////
// FixedSafe - Deref
/////////////////////////////////////////////////////////////////////////////

impl<H, R, A> Deref for FixedUnsafe<H, R, A>
    where H: Header,
          R: Fixed + Record<Header=H>,
          A: Allocator {
    type Target = R;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

/////////////////////////////////////////////////////////////////////////////
// FixedSafe - DerefMut
/////////////////////////////////////////////////////////////////////////////

impl<H, R, A> DerefMut for FixedUnsafe<H, R, A>
    where H: Header,
          R: Fixed + Record<Header=H>,
          A: Allocator {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

pub struct FixedHead<H: Header> {
    head: *const H,
    _phantom: PhantomData<H>,
}

impl<H: Header> FixedHead<H> {
    #[inline(always)]
    pub fn new(begin: *const u8) -> Self {
        Self { head: unsafe { begin as *const H }, _phantom: PhantomData }
    }

    #[inline(always)]
    pub fn new_with_size(begin: *const u8, size: usize) -> Self {
        let s = Self { head: unsafe { begin as *const H }, _phantom: PhantomData };
        (unsafe { &mut *(s.head as *mut H) }).set_size(size);
        s
    }
}

/////////////////////////////////////////////////////////////////////////////
// Message
/////////////////////////////////////////////////////////////////////////////

pub struct Flex<
    H: Header,
    R: Record<Header=H>,
    A: Allocator = Global
> {
    head: FlexHead<H>,
    base: R,
    _p: PhantomData<(H, R, A)>,
}

impl<H, R, A> Flex<H, R, A>
    where H: Header,
          R: Record<Header=H>,
          A: Allocator {
    const INITIAL_SIZE: usize = H::SIZE as usize + R::SIZE;
    const DEREF_BASE_OFFSET: isize = H::SIZE - H::Block::SIZE_BYTES;
    const BASE_OFFSET: isize = H::SIZE;
    const BASE_SIZE: isize = R::SIZE as isize;

    fn new() -> Option<Self> {
        let size = Self::INITIAL_SIZE;
        let p = A::allocate(size);
        if p == ptr::null_mut() {
            return None;
        }
        let deref_base = Base::new(unsafe { p.offset(Self::DEREF_BASE_OFFSET) });
        let base = unsafe { p.offset(Self::BASE_OFFSET) };
        let base_end = unsafe { base.offset(Self::BASE_SIZE) };
        Some(Self {
            head: FlexHead::new(p, unsafe { p.offset(size as isize) }),
            base: R::new(base, base_end, deref_base),
            _p: PhantomData,
        })
    }

    pub(crate) fn from_builder(head: *const u8, base: *const u8, _tail: *const u8, end: *const u8) -> Self {
        Self {
            head: FlexHead::new(head, end),
            base: R::new(unsafe { base.offset(H::Block::SIZE_BYTES) }, ptr::null_mut(), Base::new(base)),
            _p: PhantomData,
        }
    }
}

impl<H, R, A> Message for Flex<H, R, A>
    where H: Header,
          R: Record<Header=H>,
          A: Allocator {
    type Header = H;
    type Record = R;
    type Allocator = A;

    #[inline(always)]
    fn size(&self) -> usize {
        self.header().size()
    }

    #[inline(always)]
    fn capacity(&self) -> usize {
        unsafe { self.head.end as usize - self.head.head as usize }
    }

    #[inline(always)]
    fn header(&self) -> &Self::Header {
        unsafe { &*self.head.head }
    }

    #[inline(always)]
    fn header_mut(&mut self) -> &mut Self::Header {
        unsafe { &mut *(self.head.head as *mut Self::Header) }
    }

    #[inline(always)]
    fn base(&self) -> &Self::Record {
        &self.base
    }

    #[inline(always)]
    fn base_mut(&mut self) -> &mut Self::Record {
        &mut self.base
    }
}

/////////////////////////////////////////////////////////////////////////////
// Flex - Drop
/////////////////////////////////////////////////////////////////////////////

impl<H, R, A> Drop for Flex<H, R, A>
    where H: Header,
          R: Record<Header=H>,
          A: Allocator {
    fn drop(&mut self) {
        if self.head.head != ptr::null_mut() {
            unsafe { A::deallocate(self.head.head as *mut u8, self.capacity()); }
        }
    }
}

/////////////////////////////////////////////////////////////////////////////
// Flex - Deref
/////////////////////////////////////////////////////////////////////////////

impl<H, R, A> Deref for Flex<H, R, A>
    where H: Header,
          R: Record<Header=H>,
          A: Allocator {
    type Target = R;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

/////////////////////////////////////////////////////////////////////////////
// Flex - DerefMut
/////////////////////////////////////////////////////////////////////////////

impl<H, R, A> DerefMut for Flex<H, R, A>
    where H: Header,
          R: Record<Header=H>,
          A: Allocator {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

/////////////////////////////////////////////////////////////////////////////
// Record
/////////////////////////////////////////////////////////////////////////////

pub trait Record {
    type Header: Header;
    type Layout: Sized;

    const SIZE: usize;

    fn new(p: *const u8, bounds: *const u8, m: Base<Self::Header>) -> Self;
}

/////////////////////////////////////////////////////////////////////////////
// Head
/////////////////////////////////////////////////////////////////////////////

struct FlexHead<H: Header> {
    head: *const H,
    end: *const u8,
    _phantom: PhantomData<H>,
}

impl<H: Header> FlexHead<H> {
    #[inline(always)]
    pub fn new(begin: *const u8, end: *const u8) -> FlexHead<H> {
        FlexHead { head: unsafe { begin as *const H }, end, _phantom: PhantomData }
    }
}

/////////////////////////////////////////////////////////////////////////////
// Base16Ref
/////////////////////////////////////////////////////////////////////////////
pub struct Base<H: Header> {
    p: *const u8,
    _phantom: PhantomData<H>,
}

impl<H: Header> Base<H> {
    #[inline(always)]
    pub fn new(p: *const u8) -> Base<H> {
        Base {
            p,
            _phantom: PhantomData,
        }
    }

    #[inline(always)]
    pub fn header_ptr(&self) -> *const u8 {
        unsafe {
            self.p.offset(-H::SIZE)
        }
    }

    #[inline(always)]
    pub fn message_size(&self) -> usize {
        H::raw_size(self.header_ptr())
    }

    #[inline(always)]
    pub fn base_size(&self) -> usize {
        H::raw_size(self.p)
    }

    #[inline(always)]
    pub fn base_ptr(&self) -> *const u8 {
        unsafe { self.p }
    }

    #[inline(always)]
    pub fn offset(&self, offset: isize) -> *const u8 {
        unsafe { self.p.offset(offset) }
    }
}


#[cfg(test)]
mod tests {
    use std::mem;
    use std::ops::Deref;
    use std::ptr::slice_from_raw_parts;

    use crate::block::Block16;
    use crate::header::{Fixed32, Header16};

    use super::*;

    pub trait Order {
        fn id(&self) -> &str;

        fn set_id(&mut self) -> &mut Self;
    }

    /////////////////////////////////////////////////////////////////////////////
    // OrderMessage - helpers
    /////////////////////////////////////////////////////////////////////////////
    pub struct OrderMessage;

    impl OrderMessage {
        pub fn new() -> Option<Flex<Fixed16, OrderSafe<Fixed16>>> {
            Flex::new()
        }

        pub fn new_unsafe() -> Option<Flex<Fixed16, OrderUnsafe<Fixed16>>> {
            Flex::new()
        }

        // pub fn new_with_allocator<A: Allocator>() -> Message16<OrderSafe<Sized16>, Sized16, A> {
        //     Message16::<OrderSafe<Sized16>, Sized16, A>::new()
        // }
        //
        // pub fn new_with_header<H: Header + Header16>() -> Message16<OrderSafe<H>, H> {
        //     Message16::<OrderSafe<H>, H>::new()
        // }
        //
        // pub fn new_with_header_allocator<H: Header + Header16, A: Allocator>() -> Message16<OrderSafe<H>, H, A> {
        //     Message16::<OrderSafe<H>, H, A>::new()
        // }
    }

    /////////////////////////////////////////////////////////////////////////////
    // Order - Layout
    /////////////////////////////////////////////////////////////////////////////
    #[repr(C, packed)]
    pub struct OrderLayout {
        id: [u8; 10],
    }

    pub struct OrderSafe<H: Header + Header16 = Fixed16> {
        p: *const OrderLayout,
        base: Base<H>,
    }

    pub struct OrderUnsafe<H: Header + Header16 = Fixed16> {
        p: *const OrderLayout,
        base: Base<H>,
        bounds: *const u8,
    }

    impl<H: Header + Header16> Record for OrderSafe<H> {
        type Header = H;
        type Layout = OrderLayout;


        const SIZE: usize = core::mem::size_of::<OrderLayout>();

        fn new(p: *const u8, _: *const u8, m: Base<Self::Header>) -> Self {
            Self { p: unsafe { p as *const OrderLayout }, base: m }
        }
    }

    impl<H: Header + Header16> Record for OrderUnsafe<H> {
        type Header = H;
        type Layout = OrderLayout;

        const SIZE: usize = core::mem::size_of::<OrderLayout>();

        fn new(p: *const u8, bounds: *const u8, m: Base<Self::Header>) -> Self {
            Self { p: unsafe { p as *const OrderLayout }, base: m, bounds }
        }
    }


    impl<H: Header + Header16> Order for OrderSafe<H> {
        fn id(&self) -> &str {
            ""
        }

        fn set_id(&mut self) -> &mut Self {
            self
        }
    }

    impl<H: Header + Header16> Order for OrderUnsafe<H> {
        fn id(&self) -> &str {
            ""
        }

        fn set_id(&mut self) -> &mut Self {
            self
        }
    }

    impl<H: Header + Header16> Deref for OrderSafe<H> {
        type Target = OrderLayout;

        fn deref(&self) -> &Self::Target {
            unsafe { &*self.p }
        }
    }

    impl<H: Header + Header16> DerefMut for OrderSafe<H> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            unsafe { &mut *(self.p as *mut OrderLayout) }
        }
    }

    // impl<H: Header + Header16> Deref for OrderUnsafe<H> {
    //     type Target = Self;
    //
    //     fn deref(&self) -> &Self::Target {
    //         self
    //     }
    // }
    //
    // impl<H: Header + Header16> DerefMut for OrderUnsafe<H> {
    //     fn deref_mut(&mut self) -> &mut Self::Target {
    //         self
    //     }
    // }

    pub trait Price {
        fn wap_hash(&self) -> u64;

        fn copy_from(&mut self, to: &mut impl Price);
        fn copy_to(&self, to: &mut impl Price);

        fn open(&self) -> f64;
        fn high(&self) -> f64;
        fn low(&self) -> f64;
        fn close(&self) -> f64;

        fn set_open(&mut self, value: f64) -> &mut Self;
        fn set_high(&mut self, value: f64) -> &mut Self;
        fn set_low(&mut self, value: f64) -> &mut Self;
        fn set_close(&mut self, value: f64) -> &mut Self;
    }

    #[inline(always)]
    fn copy_price(from: &impl Price, to: &mut impl Price) {
        to.set_open(from.open())
            .set_high(from.high())
            .set_low(from.low())
            .set_close(from.close());
    }

    #[derive(Copy, Clone, Debug)]
    #[repr(C, packed)]
    pub struct PriceLayout {
        open: f64,
        high: f64,
        low: f64,
        close: f64,
    }

    impl Price for PriceLayout {
        #[inline(always)]
        fn wap_hash(&self) -> u64 {
            hash::hash_default(unsafe { self as *const Self as *const u8 }, core::mem::size_of::<PriceLayout>() as u64)
        }

        #[inline(always)]
        fn copy_from(&mut self, from: &mut impl Price) {
            copy_price(from, self);
        }

        #[inline(always)]
        fn copy_to(&self, to: &mut impl Price) {
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

    pub struct PriceSafe<H: Header> {
        p: *const PriceLayout,
        _phantom: PhantomData<H>,
    }

    impl<H: Header> Deref for PriceSafe<H> {
        type Target = PriceLayout;

        #[inline(always)]
        fn deref(&self) -> &Self::Target {
            unsafe { &*self.p }
        }
    }

    impl<H: Header> DerefMut for PriceSafe<H> {
        #[inline(always)]
        fn deref_mut(&mut self) -> &mut Self::Target {
            unsafe { &mut *(self.p as *mut PriceLayout) }
        }
    }

    impl<H: Header> Price for PriceSafe<H> {
        #[inline(always)]
        fn wap_hash(&self) -> u64 {
            hash::hash_default(unsafe { self.p as *const u8 }, core::mem::size_of::<PriceLayout>() as u64)
        }

        #[inline(always)]
        fn copy_from(&mut self, from: &mut impl Price) {
            copy_price(from, self);
        }

        #[inline(always)]
        fn copy_to(&self, to: &mut impl Price) {
            copy_price(self, to);
        }

        #[inline(always)]
        fn open(&self) -> f64 {
            unsafe { (&*self.p).open() }
        }

        #[inline(always)]
        fn high(&self) -> f64 {
            unsafe { (&*self.p).high() }
        }

        #[inline(always)]
        fn low(&self) -> f64 {
            unsafe { (&*self.p).low() }
        }

        #[inline(always)]
        fn close(&self) -> f64 {
            unsafe { (&*self.p).close() }
        }

        #[inline(always)]
        fn set_open(&mut self, value: f64) -> &mut Self {
            unsafe { (&mut *(self.p as *mut PriceLayout)).set_open(value); }
            self
        }

        #[inline(always)]
        fn set_high(&mut self, value: f64) -> &mut Self {
            unsafe { (&mut *(self.p as *mut PriceLayout)).set_open(value); }
            self
        }

        #[inline(always)]
        fn set_low(&mut self, value: f64) -> &mut Self {
            unsafe { (&mut *(self.p as *mut PriceLayout)).set_low(value); }
            self
        }

        #[inline(always)]
        fn set_close(&mut self, value: f64) -> &mut Self {
            unsafe { (&mut *(self.p as *mut PriceLayout)).set_close(value); }
            self
        }
    }

    impl<H: Header> Record for PriceSafe<H> {
        type Header = H;
        type Layout = PriceLayout;


        const SIZE: usize = core::mem::size_of::<PriceLayout>();

        fn new(p: *const u8, bounds: *const u8, m: Base<Self::Header>) -> Self {
            Self { p: unsafe { p as *const PriceLayout }, _phantom: PhantomData }
        }
    }

    pub struct PriceUnsafe<H: Header> {
        p: *const PriceLayout,
        end: *const u8,
        _phantom: PhantomData<H>,
    }

    impl<H: Header> Price for PriceUnsafe<H> {
        #[inline(always)]
        fn wap_hash(&self) -> u64 {
            hash::hash_default(unsafe { self.p as *const u8 }, unsafe { self.end as usize - (self.p as *const u8) as usize } as u64)
        }

        #[inline(always)]
        fn copy_from(&mut self, from: &mut impl Price) {
            copy_price(from, self);
        }

        #[inline(always)]
        fn copy_to(&self, to: &mut impl Price) {
            copy_price(self, to);
        }

        #[inline(always)]
        fn open(&self) -> f64 {
            if unsafe { self.p == ptr::null() || (self.p as *const u8).offset(8) > self.end } { 0f64 } else { unsafe { (&*(self.p)).open() } }
        }

        #[inline(always)]
        fn high(&self) -> f64 {
            if unsafe { self.p == ptr::null() || (self.p as *const u8).offset(16) > self.end } { 0f64 } else { unsafe { (&*(self.p)).open() } }
        }

        #[inline(always)]
        fn low(&self) -> f64 {
            if unsafe { self.p == ptr::null() || (self.p as *const u8).offset(24) > self.end } { 0f64 } else { unsafe { (&*(self.p)).low() } }
        }

        #[inline(always)]
        fn close(&self) -> f64 {
            if unsafe { self.p == ptr::null() || (self.p as *const u8).offset(32) > self.end } { 0f64 } else { unsafe { (&*(self.p)).close() } }
        }

        #[inline(always)]
        fn set_open(&mut self, value: f64) -> &mut Self {
            if unsafe { self.p == ptr::null() || (self.p as *const u8).offset(8) > self.end } { self } else {
                unsafe { (&mut *(self.p as *mut PriceLayout)).set_open(value) };
                self
            }
        }

        #[inline(always)]
        fn set_high(&mut self, value: f64) -> &mut Self {
            if unsafe { self.p == ptr::null() || (self.p as *const u8).offset(16) > self.end } { self } else {
                unsafe { (&mut *(self.p as *mut PriceLayout)).set_high(value) };
                self
            }
        }

        #[inline(always)]
        fn set_low(&mut self, value: f64) -> &mut Self {
            if unsafe { self.p == ptr::null() || (self.p as *const u8).offset(24) > self.end } { self } else {
                unsafe { (&mut *(self.p as *mut PriceLayout)).set_low(value) };
                self
            }
        }

        #[inline(always)]
        fn set_close(&mut self, value: f64) -> &mut Self {
            if unsafe { self.p == ptr::null() || (self.p as *const u8).offset(32) > self.end } { self } else {
                unsafe { (&mut *(self.p as *mut PriceLayout)).set_close(value) };
                self
            }
        }
    }

    impl Fixed for PriceLayout {}

    impl<H: Header> Fixed for PriceSafe<H> {}

    impl<H: Header> Fixed for PriceUnsafe<H> {}

    impl<H: Header> Record for PriceUnsafe<H> {
        type Header = H;
        type Layout = PriceLayout;

        const SIZE: usize = mem::size_of::<PriceLayout>();

        fn new(p: *const u8, bounds: *const u8, m: Base<Self::Header>) -> Self {
            Self { p: unsafe { p as *const PriceLayout }, end: bounds, _phantom: PhantomData }
        }
    }

    pub struct PriceMessage;

    impl PriceMessage {
        pub fn new() -> Option<FixedSafe<Fixed16, PriceSafe<Fixed16>, Global>> {
            FixedSafe::new()
        }

        pub fn new_unsafe() -> Option<FixedUnsafe<Fixed16, PriceUnsafe<Fixed16>, Global>> {
            FixedUnsafe::new()
        }

        pub fn parse(b: *const u8, length: usize) -> Option<
            impl Message<
                Header=Fixed16,
                Record=impl Record<Header=Fixed16> + Price + Fixed,
                Allocator=Global>> {
            FixedSafe::<Fixed16, PriceSafe<Fixed16>, Global>::new()
        }

        pub fn parse_with_header<H: Header>(b: *const u8, length: usize) -> Option<
            impl Message<
                Header=H,
                Record=impl Record<Header=H> + Price,
                Allocator=Global>> {
            FixedSafe::<H, PriceSafe<H>, Global>::new()
        }
    }

    fn process_order<T: Order>(order: &mut T) {
        order.set_id();
        println!("order");
    }

    fn process_order2(order: &mut impl Order) {
        order.set_id();
        println!("order");
    }

    fn d<T: Deref + DerefMut>(t: &mut T) {}

    fn process_price(price: &mut impl Price) {
        price.set_open(4198.25);
        println!("open: {:}", price.open());
    }

    #[test]
    fn message() {
        let mut price = PriceMessage::new().unwrap();
        let mut price_unsafe = PriceMessage::new_unsafe().unwrap();
        let mut price_header = price.header_mut();

        let mut p = PriceMessage::parse_with_header::<Fixed16>(ptr::null(), 34).unwrap();
        process_price(p.base_mut());

        let mut p = price.deref_mut();

        println!("PriceStruct Size: {:}", mem::size_of::<PriceLayout>());
        println!("Price Message Size: {:}", price.size());

        let mut order_safe = OrderMessage::new().unwrap();
        let mut order_unsafe = OrderMessage::new_unsafe().unwrap();

        let o = order_safe.deref_mut();
        let mut ou = order_unsafe.deref_mut();

        d(&mut ou);

        let p: &mut PriceLayout = price.deref_mut();
        p.set_open(10.0);

        let mut o = OrderMessage::new().unwrap();

        // OrderMessage::new_with_allocator::<Global>();
        // OrderMessage::new_with_header::<Sized16>();
        // OrderMessage::new_with_header_allocator::<Sized16, Global>();

        // let r = o.deref_mut().deref_mut();
        // process_order(r);
        // process_order2(r);

        // let rr = m.rec2();
        // let r = m.deref_mut();
        // let p: Price<slice::FixedUnsafe16> = Price::new(slice::FixedUnsafe::<Block16>::new(ptr::null_mut(), ptr::null_mut()));
        println!("done");
    }
}