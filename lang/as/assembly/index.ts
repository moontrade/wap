// The entry file of your WebAssembly module.

export function add(a: i32, b: i32): i32 {
    let x = heap.alloc(128);
    heap.free(x);
    return a + b;
}

@unmanaged
class Order {
    id: i32 = 0
    name: StaticArray<u8> = []
}
