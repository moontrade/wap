#![feature(test)]

extern crate anyhow;
extern crate wasmtime;
extern crate wasmtime_wasi;

use std::alloc::{alloc_zeroed, Layout};
use std::error::Error;

// use std::thread::spawn;
// use futures::executor::block_on;
use wasmtime::*;
use wasmtime_wasi::WasiCtx;

struct MyState {
    message: String,
    wasi: wasmtime_wasi::WasiCtx,
}

fn main3() -> anyhow::Result<()> {
    // An engine stores and configures global compilation settings like
    // optimization level, enabled wasm features, etc.
    let mut config = Config::new();

    let engine = Engine::default();
    let mut linker = Linker::new(&engine);
    wasmtime_wasi::add_to_linker(&mut linker, |state: &mut MyState| &mut state.wasi)?;

    // We start off by creating a `Module` which represents a compiled form
    // of our input wasm module. In this case it'll be JIT-compiled after
    // we parse the text format.
    let module = Module::from_file(&engine, "wapc.wasm")?;

    let wasi = wasmtime_wasi::sync::WasiCtxBuilder::new().
        inherit_stdio().
        inherit_args()?.build();
    // let wasi = wasmtime_wasi::sync::WasiCtxBuilder::new().
    //     inherit_stdio().
    //     inherit_args()?;

    // A `Store` is what will own instances, functions, globals, etc. All wasm
    // items are stored within a `Store`, and it's what we'll always be using to
    // interact with the wasm world. Custom data can be stored in stores but for
    // now we just use `()`.
    let mut store = Store::new(&engine, MyState {
        message: format!("hello!"),
        wasi,
    });

    linker.module(&mut store, "", &module)?;
    linker
        .get_default(&mut store, "")?
        .typed::<(), (), _>(&store)?
        .call(&mut store, ())?;

    // // With a compiled `Module` we can then instantiate it, creating
    // // an `Instance` which we can actually poke at functions on.
    // let instance = Instance::new(&mut store, &module, &[])?;
    //
    // // The `Instance` gives us access to various exported functions and items,
    // // which we access here to pull out our `answer` exported function and
    // // run it.
    // let answer = instance.get_func(&mut store, "_start")
    //     .expect("`answer` was not an exported function");
    //
    // // There's a few ways we can call the `answer` `Func` value. The easiest
    // // is to statically assert its signature with `typed` (in this case
    // // asserting it takes no arguments and returns one i32) and then call it.
    // let answer = answer.typed::<(), i32, _>(&store)?;
    //
    // // And finally we can call our function! Note that the error propagation
    // // with `?` is done to handle the case where the wasm function traps.
    // let result = answer.call(&mut store, ())?;
    // println!("Answer: {:?}", result);
    Ok(())
}

// fn main() -> anyhow::Result<()> {
//     run()
// }

fn main() -> anyhow::Result<()> {
    // Create our `store_fn` context and then compile a module and create an
    // instance from the compiled module all in one go.
    let mut store: Store<()> = Store::default();
    let module = Module::from_file(store.engine(), "memory.wat")?;
    let instance = Instance::new(&mut store, &module, &[])?;

    // load_fn up our exports from the instance
    let memory = instance
        .get_memory(&mut store, "memory")
        .ok_or(anyhow::format_err!("failed to find `memory` export"))?;
    let size = instance.get_typed_func::<(), i32, _>(&mut store, "size")?;
    let load_fn = instance.get_typed_func::<i32, i32, _>(&mut store, "load")?;
    let store_fn = instance.get_typed_func::<(i32, i32), (), _>(&mut store, "store")?;

    println!("Checking memory...");
    assert_eq!(memory.size(&store), 2);
    assert_eq!(memory.data_size(&store), 0x20000);
    assert_eq!(memory.data_mut(&mut store)[0], 0);
    assert_eq!(memory.data_mut(&mut store)[0x1000], 1);
    assert_eq!(memory.data_mut(&mut store)[0x1003], 4);

    assert_eq!(size.call(&mut store, ())?, 2);
    assert_eq!(load_fn.call(&mut store, 0)?, 0);
    assert_eq!(load_fn.call(&mut store, 0x1000)?, 1);
    assert_eq!(load_fn.call(&mut store, 0x1003)?, 4);
    assert_eq!(load_fn.call(&mut store, 0x1ffff)?, 0);
    assert!(load_fn.call(&mut store, 0x20000).is_err()); // out of bounds trap

    println!("Mutating memory...");
    memory.data_mut(&mut store)[0x1003] = 5;

    store_fn.call(&mut store, (0x1002, 6))?;
    assert!(store_fn.call(&mut store, (0x20000, 0)).is_err()); // out of bounds trap

    assert_eq!(memory.data(&store)[0x1002], 6);
    assert_eq!(memory.data(&store)[0x1003], 5);
    assert_eq!(load_fn.call(&mut store, 0x1002)?, 6);
    assert_eq!(load_fn.call(&mut store, 0x1003)?, 5);

    // Grow memory.
    println!("Growing memory...");
    memory.grow(&mut store, 1)?;
    assert_eq!(memory.size(&store), 3);
    assert_eq!(memory.data_size(&store), 0x30000);

    assert_eq!(load_fn.call(&mut store, 0x20000)?, 0);
    store_fn.call(&mut store, (0x20000, 0))?;
    assert!(load_fn.call(&mut store, 0x30000).is_err());
    assert!(store_fn.call(&mut store, (0x30000, 0)).is_err());

    assert!(memory.grow(&mut store, 1).is_err());
    assert!(memory.grow(&mut store, 0).is_ok());

    println!("Creating stand-alone memory...");
    let memorytype = MemoryType::new(5, Some(5));
    let memory2 = Memory::new(&mut store, memorytype)?;
    assert_eq!(memory2.size(&store), 5);
    assert!(memory2.grow(&mut store, 1).is_err());
    assert!(memory2.grow(&mut store, 0).is_ok());

    Ok(())
}

#[cfg(test)]
mod tests {
    extern crate anyhow;
    extern crate test;
    extern crate wasmtime;
    // extern crate wasmedge_sys;

    use wasmtime::*;
    use test::Bencher;

    #[test]
    fn run_wasi() {
        run_wasi0();
    }

    fn run_wasi0() -> anyhow::Result<()> {
        struct MyState {
            message: String,
            wasi: wasmtime_wasi::WasiCtx,
        }

        // An engine stores and configures global compilation settings like
        // optimization level, enabled wasm features, etc.
        let mut config = Config::new();

        let engine = Engine::default();
        let mut linker = Linker::new(&engine);
        wasmtime_wasi::add_to_linker(&mut linker, |state: &mut MyState| &mut state.wasi)?;

        // We start off by creating a `Module` which represents a compiled form
        // of our input wasm module. In this case it'll be JIT-compiled after
        // we parse the text format.
        let module = Module::from_file(&engine, "wapc.wasm")?;

        let wasi = wasmtime_wasi::sync::WasiCtxBuilder::new().
            inherit_stdio().
            inherit_args()?.build();
        // let wasi = wasmtime_wasi::sync::WasiCtxBuilder::new().
        //     inherit_stdio().
        //     inherit_args()?;

        // A `Store` is what will own instances, functions, globals, etc. All wasm
        // items are stored within a `Store`, and it's what we'll always be using to
        // interact with the wasm world. Custom data can be stored in stores but for
        // now we just use `()`.
        let mut store = Store::new(&engine, MyState {
            message: format!("hello!"),
            wasi,
        });

        linker.module(&mut store, "", &module)?;
        linker
            .get_default(&mut store, "")?
            .typed::<(), (), _>(&store)?
            .call(&mut store, ())?;

        // // With a compiled `Module` we can then instantiate it, creating
        // // an `Instance` which we can actually poke at functions on.
        // let instance = Instance::new(&mut store, &module, &[])?;
        //
        // // The `Instance` gives us access to various exported functions and items,
        // // which we access here to pull out our `answer` exported function and
        // // run it.
        // let answer = instance.get_func(&mut store, "_start")
        //     .expect("`answer` was not an exported function");
        //
        // // There's a few ways we can call the `answer` `Func` value. The easiest
        // // is to statically assert its signature with `typed` (in this case
        // // asserting it takes no arguments and returns one i32) and then call it.
        // let answer = answer.typed::<(), i32, _>(&store)?;
        //
        // // And finally we can call our function! Note that the error propagation
        // // with `?` is done to handle the case where the wasm function traps.
        // let result = answer.call(&mut store, ())?;
        // println!("Answer: {:?}", result);
        Ok(())
    }

    #[test]
    fn basic() {}


    #[bench]
    fn bench_call(b: &mut Bencher) {
        use wasmtime::*;
// Create our `store_fn` context and then compile a module and create an
        // instance from the compiled module all in one go.
        let mut store: Store<()> = Store::default();
        let module = Module::from_file(store.engine(), "memory.wat").unwrap();
        let instance = Instance::new(&mut store, &module, &[]).unwrap();

        // load_fn up our exports from the instance
        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or(anyhow::format_err!("failed to find `memory` export")).unwrap();
        let size = instance.get_typed_func::<(), i32, _>(&mut store, "size").unwrap();
        let load_fn = instance.get_typed_func::<i32, i32, _>(&mut store, "load").unwrap();
        let store_fn = instance.get_typed_func::<(i32, i32), (), _>(&mut store, "store").unwrap();

        b.iter(|| {
            load_fn.call(&mut store, 0x1002).unwrap();
            // store_fn.call(&mut store, (0x30000, 0)).unwrap();
        });
    }

    // #[test]
    // fn wasmedge_bench() {
    //     use wasmedge_sys::Loader;
    //     use std::path::PathBuf;
    //     use wasmedge_sys::{Config, Store, Vm};
    //
    //     // create a Loader context
    //     let loader = Loader::create(None).expect("fail to create a Loader context");
    //
    //     // load a wasm module from a specified wasm file, and return a WasmEdge AST Module instance
    //     let path = PathBuf::from("memory.wasm");
    //     let module = loader.from_file(path).expect("fail to load the WebAssembly file");
    //
    //     // create a Config context
    //     let config = Config::create().expect("fail to create a Config context");
    //
    //     // create a Store context
    //     let mut store = Store::create().expect("fail to create a Store context");
    //
    //     // create a Vm context with the given Config and Store
    //     let mut vm = Vm::create(Some(config), Some(&mut store)).expect("fail to create a Vm context");
    //
    //     use wasmedge_sys::Value;
    //
    //     // run a function
    //     let returns = vm.run_wasm_from_module(module, "load", [Value::from_i32(5)]).expect("fail to run the target function in the module");
    //
    //     println!("The result of fib(5) is {}", returns[0].to_i32());
    // }
}