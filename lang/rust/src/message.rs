use core::marker::PhantomData;
use core::ptr;
use std::cell::UnsafeCell;
use std::cmp::max;
use std::cmp::Ordering::Greater;
use std::ops::DerefMut;

use crate::{block, hash};
use crate::alloc::{Allocator, Global};
use crate::block::{Block, Block16};
use crate::hash::Hasher;
use crate::header::{Header, Header16, Sized16};

/////////////////////////////////////////////////////////////////////////////
// Message16
/////////////////////////////////////////////////////////////////////////////

pub struct Message<
    H: Header,
    R: Record<Header=H>,
    A: Allocator = Global
> {
    header: MessageRef<H>,
    record: R,
    _p: PhantomData<(H, R, A)>,
}

impl<H, R, A> Message<H, R, A>
    where H: Header,
          R: Record<Header=H>,
          A: Allocator {
    fn new() -> Self {
        let size = H::SIZE as usize + R::SIZE + 2;
        let p = unsafe { A::allocate(size) };
        let r = unsafe { p.offset(H::SIZE + 2) };
        Self {
            header: MessageRef::new(p),
            record: R::new(r, unsafe { r.offset(R::SIZE as isize) }, Base::new(p)),
            _p: PhantomData,
        }
    }

    #[inline(always)]
    fn size(&self) -> usize {
        H::size(self.header.p)
    }

    #[inline(always)]
    fn header(&self) -> &MessageRef<H> {
        &self.header
    }

    #[inline(always)]
    fn header_mut(&mut self) -> &mut MessageRef<H> {
        &mut self.header
    }

    #[inline(always)]
    fn record(&self) -> &R {
        &self.record
    }

    #[inline(always)]
    fn record_mut(&mut self) -> &mut R {
        &mut self.record
    }
}

/////////////////////////////////////////////////////////////////////////////
// Message16 - Drop
/////////////////////////////////////////////////////////////////////////////

impl<H, R, A> Drop for Message<H, R, A>
    where H: Header,
          R: Record<Header=H>,
          A: Allocator {
    fn drop(&mut self) {
        unsafe { A::deallocate(self.header.p as *mut u8, self.size()); }
    }
}

/////////////////////////////////////////////////////////////////////////////
// Message16 - Deref
/////////////////////////////////////////////////////////////////////////////

impl<H, R, A> std::ops::Deref for Message<H, R, A>
    where H: Header,
          R: Record<Header=H>,
          A: Allocator {
    type Target = R;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.record
    }
}

/////////////////////////////////////////////////////////////////////////////
// Message16 - DerefMut
/////////////////////////////////////////////////////////////////////////////

impl<H, R, A> std::ops::DerefMut for Message<H, R, A>
    where H: Header,
          R: Record<Header=H>,
          A: Allocator {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.record
    }
}

/////////////////////////////////////////////////////////////////////////////
// Struct16
/////////////////////////////////////////////////////////////////////////////

pub trait Record {
    type Header: Header;

    const SIZE: usize;

    fn new(p: *const u8, bounds: *const u8, m: Base<Self::Header>) -> Self;
}

/////////////////////////////////////////////////////////////////////////////
// Header16Ref
/////////////////////////////////////////////////////////////////////////////

pub struct MessageRef<H: Header> {
    p: *const u8,
    _phantom: PhantomData<H>,
}

impl<H: Header> MessageRef<H> {
    #[inline(always)]
    pub fn new(p: *const u8) -> MessageRef<H> {
        MessageRef { p, _phantom: PhantomData }
    }

    #[inline(always)]
    pub fn size(&self) -> usize {
        H::size(self.p)
    }

    #[inline(always)]
    pub fn base(&self) -> *const u8 {
        unsafe { self.p.offset(H::SIZE) }
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
        H::size(self.header_ptr())
    }

    #[inline(always)]
    pub fn base_size(&self) -> usize {
        H::size(self.p)
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
    use std::ops::Deref;

    use crate::block::Block16;
    use crate::header::{Header16, Sized32};

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
        pub fn new() -> Message<Sized16, OrderSafe<Sized16>> {
            Message::<Sized16, OrderSafe<Sized16>>::new()
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
    pub struct Order_ {
        id: [u8; 10],
    }

    pub struct OrderSafe<H: Header + Header16 = Sized16> {
        p: *const Order_,
        m: Base<H>,
    }

    pub struct OrderUnsafe<H: Header + Header16 = Sized16> {
        p: *const Order_,
        m: Base<H>,
        bounds: *const u8,
    }

    impl<H: Header + Header16> Record for OrderSafe<H> {
        type Header = H;
        const SIZE: usize = core::mem::size_of::<Order_>();

        fn new(p: *const u8, _: *const u8, m: Base<Self::Header>) -> Self {
            Self { p: unsafe { p as *const Order_ }, m }
        }
    }

    impl<H: Header + Header16> Record for OrderUnsafe<H> {
        type Header = H;
        const SIZE: usize = core::mem::size_of::<Order_>();

        fn new(p: *const u8, bounds: *const u8, m: Base<Self::Header>) -> Self {
            Self { p: unsafe { p as *const Order_ }, m, bounds }
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

    pub trait Price {
        fn as_ptr(&self) -> *const u8;

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
    pub struct Price_ {
        open: f64,
        high: f64,
        low: f64,
        close: f64,
    }

    impl Price for Price_ {
        #[inline(always)]
        fn as_ptr(&self) -> *const u8 {
            unsafe { self as *const Self as *const u8 }
        }

        #[inline(always)]
        fn wap_hash(&self) -> u64 {
            hash::hash_default(self.as_ptr(), core::mem::size_of::<Price_>() as u64)
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

    impl Hasher for Price_ {
        fn hash_wap(self) -> u64 {
            hash::hash_default(self.as_ptr(), 32)
        }
    }

    pub struct PriceUnsafe {
        p: *const Price_,
        end: *const u8,
    }

    impl Price for PriceUnsafe {
        #[inline(always)]
        fn as_ptr(&self) -> *const u8 {
            unsafe { self.p as *const u8 }
        }

        #[inline(always)]
        fn wap_hash(&self) -> u64 {
            hash::hash_default(self.as_ptr(), unsafe { self.end as usize - (self.p as *const u8) as usize } as u64)
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
                unsafe { (&mut *(self.p as *mut Price_)).set_open(value) };
                self
            }
        }

        #[inline(always)]
        fn set_high(&mut self, value: f64) -> &mut Self {
            if unsafe { self.p == ptr::null() || (self.p as *const u8).offset(16) > self.end } { self } else {
                unsafe { (&mut *(self.p as *mut Price_)).set_high(value) };
                self
            }
        }

        #[inline(always)]
        fn set_low(&mut self, value: f64) -> &mut Self {
            if unsafe { self.p == ptr::null() || (self.p as *const u8).offset(24) > self.end } { self } else {
                unsafe { (&mut *(self.p as *mut Price_)).set_low(value) };
                self
            }
        }

        #[inline(always)]
        fn set_close(&mut self, value: f64) -> &mut Self {
            if unsafe { self.p == ptr::null() || (self.p as *const u8).offset(32) > self.end } { self } else {
                unsafe { (&mut *(self.p as *mut Price_)).set_close(value) };
                self
            }
        }
    }

    impl Hasher for PriceUnsafe {
        fn hash_wap(self) -> u64 {
            hash::hash_default(self.as_ptr(), unsafe { self.end as usize - (self.p as *const u8) as usize } as u64)
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

    #[test]
    fn message() {
        println!("{:}", 0);

        let mut o = OrderMessage::new();
        // OrderMessage::new_with_allocator::<Global>();
        // OrderMessage::new_with_header::<Sized16>();
        // OrderMessage::new_with_header_allocator::<Sized16, Global>();

        let r = o.record_mut();
        process_order(r);
        process_order2(r);

        // let rr = m.rec2();
        // let r = m.deref_mut();
        // let p: Price<slice::FixedUnsafe16> = Price::new(slice::FixedUnsafe::<Block16>::new(ptr::null_mut(), ptr::null_mut()));
        println!("done");
    }
}