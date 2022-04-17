# WAP - WASM Proto

Language / platform agnostic wire-safe linear (flat) memory rich data models.

## Single Byte
####
    bool
    i8
    u8

## Little Endian
####
    i16
    i32
    i64
    u16
    u32
    u64
    f32
    f64

## Big Endian
####
    i16b
    i32b
    i64b
    u16b
    u32b
    u64b
    f32b
    f64b

## Native Endian (not wire-safe)
####
    i16n
    i32n
    i64n
    u16n
    u32n
    u64n
    f32n
    f64n

## String

##### example:
    string

## Inline String
Embedded string with a max total size including length field.
#### template:
    string${size}
#### example:
    string8

## String Variant
#### Shorthand for defining a variant that is either an inline string or a pointer to a string:
    variant vstring8 {
        string8
        string
    }
#### example:
    string8...

## array
#### template:
    [ ${size} ] ${type}
#### example:
    [8]i64

## vector
#### example:
    []i64

## map
Robinhood hashmap (linear probing)
#### example:
    map<i64, string>

## sorted map
ART Radix Tree
#### example:
    sorted_map<i64, string>

## tree map
B+Tree
#### example:
    tree_map<i64, string>

## set
Robinhood hashmap with no values
#### example:
    set<i64>

## sorted set
ART Radix Tree with no values
#### example:
    sorted_set<i64>

## tree set
B+Tree with no values
#### example:
    tree_set<i64>

## enum
####
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

####
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
####
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

## Views and Builders (Immutable and Mutable) 

## Bounds Checking Optimizations
Only pay for bounds checking when necessary.
#### modes:
1. Trusted - Message was built with current generated code. No bounds checking at all. FAST!
2. Partial - Use a view/builder variant for each internal struct root. Wire mode optimization.
3. Untrusted - Perform bounds check on every access. Wire mode default.

Builders default to Trusted mode.

Trusted mode should be competitive with raw struct access in the following languages:
- C/C++
- Rust
- Go
- AssemblyScript

## JSON

Optionally supports JSON serialization and deserialization. Can utilize WAP for in-memory structure and JSON
for the wire protocol.

## Protocol Buffers

Optionally supports Protocol Buffers serialization and deserialization.

## Reducing GC / Allocator Pressure

A WAP root message is a single contiguous chunk of memory regardless of the number of internal structures there are.

## Evolutions
Provide field number tag '@'
####
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

## Builder Allocators
