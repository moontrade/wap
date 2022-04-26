// use futures::executor::block_on;
#![feature(test)]

// extern crate wee_alloc;

use std::alloc::{alloc, dealloc, GlobalAlloc, Layout};
use std::future::Future;
use futures::executor::block_on;

mod hash;

// Use `wee_alloc` as the global allocator.
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

// `foo()` returns a type that implements `Future<Output = u8>`.
// `foo().await` will result in a value of type `u8`.
async fn foo() -> u8 {
    println!("foo()");
    5
}

fn bar() -> impl Future<Output=u8> {
    // This `async` block results in a type that implements
    // `Future<Output = u8>`.
    async {
        let x: u8 = foo().await;
        x + 5
    }
}

async fn count() {
    use futures::{future, select};

    let mut a_fut = future::ready(4);
    let mut b_fut = future::ready(6);
    let mut total = 0;

    loop {
        select! {
            a = a_fut => {
                total += a
            },
            b = b_fut => total += b,
            complete => break,
            default => unreachable!(), // never runs (futures are ready, then complete)
        }
    }
    println!("total: {:}", total);
    assert_eq!(total, 10);
}

fn print_hash(s: &str) {
    use hash::Hasher;
    println!("{}: {}", s, s.hash_wap());
}

pub fn main() {
    print_hash("h");
    print_hash("he");
    print_hash("hel");
    print_hash("hell");
    print_hash("hello");
    print_hash("hellonow");
    print_hash("hellonowhellonow");
    print_hash("hellonowhellonowhellonowhellonow");
    print_hash("hellonowhellonowhellonowhellonowhellonowhellonowhellonowhellonow");

    let p = unsafe { alloc(Layout::from_size_align_unchecked(64, 8)) };
    println!("alloc pointer: {:}", p as usize);
    unsafe { dealloc(p, Layout::from_size_align_unchecked(64, 8)); }

    println!("hash_u64(10): {:}", hash::hash_u64(10));

    block_on(async {
        count().await;

        let _br = bar().await;
    });
}