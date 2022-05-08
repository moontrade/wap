# WAP - WebAssembly Proto

## Expressive Flat Data Structures
Language / platform-agnostic wire-safe linear (flat) memory rich data models.

## Dramatically reduce allocations
####


## Stop parsing so dang much


# Data Types

------------------------------------------------
## Single Byte
### types:
    bool
    i8
    u8

## Little Endian
### types:
    i16
    i32
    i64
    i128
    u16
    u32
    u64
    u128
    f32
    f64

## Big Endian
### types:
    i16b
    i32b
    i64b
    i128b
    u16b
    u32b
    u64b
    u128b
    f32b
    f64b

## Native Endian (not wire-safe)
### types:
    i16n
    i32n
    i64n
    i128n
    u16n
    u32n
    u64n
    u128n
    f32n
    f64n

## Pointer
Pointers are an unsigned integer that represents the offset in the buffer where the data type resides or 0 for null.
### sizes
    usize16 - Supports messages <64kb
    usize32 - Supports messages <4GB
    usize64 - Supports messages >4GB

## String

### example:
    string

## Inline String
Embedded string with a max total size including length field.
### template:
    string{size}
### example:
    string8

## String Variant (small string optimization)
Inline string with heap allocated spill-over when string size is larger than inline size.
### template:
    string{size}..
### example:
    string8..
### shorthand for:
    variant vstring8 {
        string8
        string
    }

## array
### template:
    [{size}]{type}
### example:
    [8]i64

## vector
### example:
    []i64

## map
Robinhood hashmap (linear probing)
### example:
    map<i64, string>

## sorted map
ART Radix Tree
### example:
    sorted_map<i64, string>

## tree map
B+Tree
### example:
    tree_map<i64, string>

## set
Robinhood hashmap with no values
### example:
    set<i64>

## sorted set
ART Radix Tree with no values
### example:
    sorted_set<i64>

## tree set
B+Tree with no values
### example:
    tree_set<i64>

## enum
###
    enum Code : i32 {
        SUCCESS = 0
        ERROR = 1
    }

    // single line using comma separator
    enum Code : i32 { SUCCESS = 0, ERROR = 1}

    // single line using semicolon separator
    enum Code : i32 { SUCCESS = 0; ERROR = 1}

## variant
Type safe union or tagged union.
- C++ = std::variant
- Rust = enum 
- Kotlin = sealed class

###
    variant Value {
        i64
        f64
        string8
        string
        // inline embedded struct
        Option {
        }
        // inline optional struct or struct pointer
        *Option {
        }
    }

## struct
### example:
    struct Order {
        uid     string  = "optional default value"
        number  u64     = 1
        // inline struct
        lines []struct{
            number      u32
            quantity    u32
            price       f64
        } = [{1, 1, 9.99}]

        // unions are declared inline and support multiple levels of nesting
        union {
            w f64
            x f64
            union {
                y f64
                z f64
            }
        }

        // inner struct = Order::Line
        struct Line {
            number      u32
            quantity    u32
            price       f64
        }
    }

## Readers and Builders (Immutable and Mutable)

## Mutable Readers (kind of like a Builder)

#### Pointer stability enforcement:
    May only modify fixed fields OR where the update will not require the underlying buffer to be resized.

## JSON

Optionally supports JSON serialization and deserialization. Can utilize WAP for in-memory structure and JSON
for the wire protocol.

## Protocol Buffers

Optionally supports Protocol Buffers serialization and deserialization.

## Reducing GC / Allocator Pressure

A WAP root message is a single contiguous chunk of memory regardless of the number of internal structures there are.

## Evolutions
Provide field number tag '@'
###
    struct Order {
        @1  uid         string
        @3  new_field   bool
        @2  number      u64
    }
     struct Order {
        uid         string
        number      u64
        new_field   bool // implicitly becomes @3
    }

## Builders

### Variable size messages only

Fixed sized messages use Mutable Reader instead. 

### Appender
Messages are built in an append-only mode. Fastest.

### Heap
Messages are built using an internal dynamic memory allocator with size classes and freelists. Best for highly mutable/dynamic messages.

#### Segregated Fit (Real-time)

---
## Rust Implementation

### Bounds Checking Elimination
Only pay for bounds checking when necessary.

---
## C++ Implementation

### Single Header File
Generated code only depends on a single header file which can be tweaked for corner use cases.

### No Virtual Methods
Virtual methods are never used for accessing data fields. Field access can be inlined in most cases.

### Bounds Checking Elimination
Only pay for bounds checking when necessary.

#### modes:
1. Struct - Message was built / guaranteed to conform with current generated code. No bounds checking at all. FAST!
2. Reader - Perform bounds check on every access. Wire mode default.
3. Reader Variant (visit) - Use a reader/builder variant for each internal struct root. Wire mode optimization. Potential partial bounds check elimination.
4. Builder - No bounds checking, however for flexible messages there will likely be another pointer dereference due to underlying buffer pointer instability when resize is required. For fixed sized messages, it's the same as Struct.

---
## Go Implementation

### Bounds Checking Optimizations

#### modes:
1. Struct - Message was built / guaranteed to conform with current generated code. No bounds checking at all. FAST!
2. Reader / Mutable Reader - Perform bounds check on every access. Wire mode default.
3. Builder - No bounds checking, however for flexible messages there will likely be another pointer dereference due to underlying buffer pointer instability when resize is required. For fixed sized messages, it's the same as Struct.

### Why not Generics?
Go generics are non-monomorphisable for pointer types. Huge runtime performance penalty that goes against WAP core performance tenants of static over dynamic.

### TinyGo Support
TinyGo is supported including WASM!

## AssemblyScript



