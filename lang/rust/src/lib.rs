#![feature(test)]
#![feature(type_alias_impl_trait)]

// mod main;

// extern crate wee_alloc;

// Use `wee_alloc` as the global allocator.
// #[global_allocator]
// static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

mod alloc;
mod hash;
mod block;
mod header;
mod message;
mod builder;
mod heap;
mod vector;
mod map;
mod string;

#[cfg(test)]
mod tests {
    use std::alloc::{alloc_zeroed, Layout};
    // use std::thread::spawn;
    // use futures::executor::block_on;
    use super::*;

    #[test]
    fn basic() {

        // let d = unsafe { alloc_zeroed(Layout::from_size_align_unchecked(128, 8)) };
        //
        // let slice = slice::FixedUnsafe::<slice::Block16>::new(d, unsafe { d.offset(128) });
        //
        // let mut order = OrderView::new(slice);
        // process_order(&mut order);
        // let o = unsafe { &*order.cast() };
        //
        // if o.id == 0 {
        //     println!("hi");
        // }
        // println!("bye");


        // let result = 2 + 2;
        // assert_eq!(result, 4);
    }
}
