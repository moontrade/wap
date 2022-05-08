/*
** Copyright (c) 2022, Clay Molocznik
** All rights reserved.
**
** Redistribution and use in source and binary forms, with or without
** modification, are permitted provided that the following conditions are met:
**     * Redistributions of source code must retain the above copyright
**       notice, this list of conditions and the following disclaimer.
**     * Redistributions in binary form must reproduce the above copyright
**       notice, this list of conditions and the following disclaimer in the
**       documentation and/or other materials provided with the distribution.
**     * Neither the name of the copyright holder nor the
**       names of its contributors may be used to endorse or promote products
**       derived from this software without specific prior written permission.
**
** THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND
** ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED
** WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
** DISCLAIMED. IN NO EVENT SHALL MATTHEW CONTE BE LIABLE FOR ANY
** DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES
** (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;
** LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND
** ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
** (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS
** SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
*/

#ifndef WAP_H
#define WAP_H

//#include <bit>
//#include <memory>
#include <optional>
//#include <numeric>
#include <string>

#define WAP_IS_BIG_ENDIAN (*(unsigned short *)"\0\xff" < 0x0100)

#if defined(__EMSCRIPTEN__)
#endif

// enum
//{
//     WAP_LITTLE_ENDIAN = 0x03020100ul,
//     WAP_BIG_ENDIAN = 0x00010203ul,
//     WAP_PDP_ENDIAN = 0x01000302ul,      /* DEC PDP-11 (aka ENDIAN_LITTLE_WORD) */
//     WAP_HONEYWELL_ENDIAN = 0x02030001ul /* Honeywell 316 (aka ENDIAN_BIG_WORD) */
// };
//
// static const union { unsigned char bytes[4]; uint32_t value; } o32_host_order =
//         { { 0, 1, 2, 3 } };
//#define WAP_HOST_ENDIAN (o32_host_order.value)

// enum wap_endian_t : unsigned int {
//     WAP_LITTLE_ENDIAN = 0x00000001,
//     WAP_BIG_ENDIAN = 0x01000000,
//     WAP_PDP_ENDIAN = 0x00010000,
//     WAP_UNKNOWN_ENDIAN = 0xFFFFFFFF
// };
//
// constexpr wap_endian_t wap_endian_order() {
//     if ((0xFFFFFFFF & 1) == WAP_LITTLE_ENDIAN) return WAP_LITTLE_ENDIAN;
//     else if ((0xFFFFFFFF & 1) == WAP_BIG_ENDIAN) return WAP_BIG_ENDIAN;
//     else if ((0xFFFFFFFF & 1) == WAP_PDP_ENDIAN) return WAP_PDP_ENDIAN;
//     else return WAP_UNKNOWN_ENDIAN;
// }

#ifndef WAP_LIKELY
#  if (defined(__GNUC__) || __has_builtin(__builtin_expect)) && !defined(__COVERITY__)
#    define WAP_LIKELY(cond) __builtin_expect(!!(cond), 1)
#  else
#    define WAP_LIKELY(x) (!!(x))
#  endif
#endif /* likely */

#ifndef WAP_UNLIKELY
#  if (defined(__GNUC__) || __has_builtin(__builtin_expect)) && !defined(__COVERITY__)
#    define WAP_UNLIKELY(cond) __builtin_expect(!!(cond), 0)
#  else
#    define WAP_UNLIKELY(x) (!!(x))
#  endif
#endif /* WAP_UNLIKELY */

#ifdef _MSC_VER

#  include <stdlib.h>
#  define bswap_16(x) _byteswap_ushort(x)
#  define bswap_32(x) _byteswap_ulong(x)
#  define bswap_64(x) _byteswap_uint64(x)

#elif defined(__APPLE__)

// Mac OS X / Darwin features
#  include <libkern/OSByteOrder.h>

#  define bswap_16(x) OSSwapInt16(x)
#  define bswap_32(x) OSSwapInt32(x)
#  define bswap_64(x) OSSwapInt64(x)

#elif defined(__sun) || defined(sun)

#  include <sys/byteorder.h>
#  define bswap_16(x) BSWAP_16(x)
#  define bswap_32(x) BSWAP_32(x)
#  define bswap_64(x) BSWAP_64(x)

#elif defined(__FreeBSD__)

#  include <sys/endian.h>
#  define bswap_16(x) bswap16(x)
#  define bswap_32(x) bswap32(x)
#  define bswap_64(x) bswap64(x)

#elif defined(__OpenBSD__)

#  include <sys/types.h>
#  define bswap_16(x) swap16(x)
#  define bswap_32(x) swap32(x)
#  define bswap_64(x) swap64(x)

#elif defined(__NetBSD__)

#  include <machine/bswap.h>
#  include <sys/types.h>
#  if defined(__BSWAP_RENAME) && !defined(__bswap_32)
#    define bswap_16(x) bswap16(x)
#    define bswap_32(x) bswap32(x)
#    define bswap_64(x) bswap64(x)
#  endif

#else

//#include <byteswap.h>

#  define bswap_16(x) ((uint16_t)((((uint16_t)(x)&0xff00U) >> 8) | (((uint16_t)(x)&0x00ffU) << 8)))

#  define bswap_32(x)                                                                    \
    ((uint32_t)((((uint32_t)(x)&0xff000000U) >> 24) | (((uint32_t)(x)&0x00ff0000U) >> 8) \
                | (((uint32_t)(x)&0x0000ff00U) << 8) | (((uint32_t)(x)&0x000000ffU) << 24)))

#  define bswap_64(x)                                           \
    ((uint64_t)((((uint64_t)(x)&0xff00000000000000ULL) >> 56)   \
                | (((uint64_t)(x)&0x00ff000000000000ULL) >> 40) \
                | (((uint64_t)(x)&0x0000ff0000000000ULL) >> 24) \
                | (((uint64_t)(x)&0x000000ff00000000ULL) >> 8)  \
                | (((uint64_t)(x)&0x00000000ff000000ULL) << 8)  \
                | (((uint64_t)(x)&0x0000000000ff0000ULL) << 24) \
                | (((uint64_t)(x)&0x000000000000ff00ULL) << 40) \
                | (((uint64_t)(x)&0x00000000000000ffULL) << 56)))

#endif

#define WAP_NO_COPY(clazz)       \
private:                         \
  clazz(const clazz &) = delete; \
  clazz &operator=(const clazz &) = delete;

#define WAP_BUF_CLASS template <typename Allocator = ::wap::allocator>
#define WAP_RDR_CLASS(clazz) clazz<Allocator>
#define WAP_RD_END(clazz) \
  }                       \
  ;

#define WAP_RDR_BODY(clazz)                         \
public:                                             \
  using Buffer = typename ::wap::buffer<Allocator>; \
  using Slice = typename Buffer::slice;             \
  explicit operator bool() { return true; }         \
                                                    \
protected:                                          \
  typename Buffer::slice s_;                        \
                                                    \
public:                                             \
  explicit clazz(Slice s) noexcept : s_(s) {}       \
  clazz(const clazz &c) noexcept : s_(c.s_) {}      \
                                                    \
public:

#define WAP_BDR_BODY(clazz, reader_clazz)                                             \
public:                                                                               \
  clazz(const clazz &b) noexcept : reader_clazz<Allocator>(b.s_) {}                   \
  explicit clazz(reader_clazz<Allocator> &r) noexcept : reader_clazz<Allocator>(r) {} \
  explicit Builder(typename ::wap::buffer<Allocator>::slice s) noexcept               \
      : reader_clazz<Allocator>(s) {}                                                 \
  reader_clazz<Allocator> to_reader() noexcept { return reader_clazz<Allocator>(this->s_); }

#define WAP_MSG_DECL(name, clazz, type, default_size, base_size, ...)                          \
public:                                                                                        \
  constexpr static const ::wap::u16 TYPE = type;                                               \
  constexpr static const ::wap::size_t DEFAULT_SIZE = default_size;                            \
  constexpr static const ::wap::size_t BASE_SIZE = base_size;                                  \
  constexpr static const ::wap::u8 DEFAULT[] = __VA_ARGS__;                                    \
  template <typename Allocator> class Reader;                                                  \
  template <typename Allocator> class Builder;                                                 \
  template <typename Allocator = ::wap::allocator>                                             \
  static ::wap::message<clazz, Allocator, Reader<Allocator>, Builder<Allocator>> make_with(    \
      Allocator allocator = Allocator(), ::wap::size_t extra = BASE_SIZE) {                    \
    return ::wap::message<clazz, Allocator, Reader<Allocator>, Builder<Allocator>>(allocator,  \
                                                                                   extra);     \
  }                                                                                            \
  static ::wap::message<clazz, ::wap::allocator::noop, Reader<::wap::allocator::noop>,   \
                        Builder<::wap::allocator::noop>>                                    \
  unmanaged(void *data, ::wap::size_t size) {                                                  \
    return ::wap::message<clazz, ::wap::allocator::noop, Reader<::wap::allocator::noop>, \
                          Builder<::wap::allocator::noop>>(                                 \
        ::wap::buffer<::wap::allocator::noop>((wap::u8 *)data, size));                      \
  }                                                                                            \
  template <typename Allocator = ::wap::allocator>                                             \
  static ::wap::message<clazz, Allocator, Reader<Allocator>, Builder<Allocator>> make(         \
      ::wap::size_t extra = BASE_SIZE, Allocator allocator = Allocator()) {                    \
    return ::wap::message<clazz, Allocator, Reader<Allocator>, Builder<Allocator>>(allocator,  \
                                                                                   extra);     \
  }

#define WAP_GET(name, type_name, offset, bounds) \
  inline ::wap::type_name name() noexcept { return s_.type_name(offset, bounds); }

#define WAP_GET_NESTED(name, type_name, offset, bounds)         \
  inline type_name<Allocator> name() noexcept {                 \
    return type_name<Allocator>(this->s_.base(offset, bounds)); \
  }

#define WAP_SET(name, type_name, offset, bounds)       \
  Builder &set_##name(wap::type_name value) noexcept { \
    this->s_.put_##type_name(offset, bounds, value);   \
    return *this;                                      \
  }

#define WAP_SLICE_ONE_BYTE(type_name)                                                             \
  inline ::wap::type_name type_name(::wap::u32 offset, ::wap::u32 bounds) {                       \
    if (WAP_UNLIKELY(bounds > size())) return 0;                                                  \
    return *(::wap::type_name *)slice_ + offset;                                                  \
  }                                                                                               \
  inline ::wap::type_name type_name##_u(::wap::u32 offset) {                                      \
    return *(::wap::type_name *)slice_ + offset;                                                  \
  }                                                                                               \
  inline ::wap::type_name type_name(::wap::u32 offset, ::wap::u32 bounds, ::wap::type_name def) { \
    if (WAP_UNLIKELY(bounds > size())) return def;                                                \
    return *(::wap::type_name *)slice_ + offset;                                                  \
  }                                                                                               \
  inline void put_##type_name(::wap::u32 offset, ::wap::u32 bounds, ::wap::type_name val) {       \
    if (WAP_UNLIKELY(bounds > size())) return;                                                    \
    *((::wap::type_name *)slice_ + offset) = val;                                                 \
  }                                                                                               \
  inline void put_##type_name##_u(::wap::u32 offset, ::wap::type_name val) {                      \
    *((::wap::type_name *)slice_ + offset) = val;                                                 \
  }

#define WAP_SLICE_ENDIANNESS(type_name)                                                            \
  inline ::wap::type_name type_name(::wap::u32 offset, ::wap::size_t bounds) const noexcept {      \
    if (WAP_UNLIKELY(bounds > size())) return 0;                                                   \
    return ::wap::bits::le::type_name(slice_ + offset);                                            \
  }                                                                                                \
  inline ::wap::type_name type_name##_u(::wap::u32 offset) const noexcept {                        \
    return ::wap::bits::le::type_name(slice_ + offset);                                            \
  }                                                                                                \
  inline ::wap::type_name type_name(::wap::u32 offset, ::wap::size_t bounds, ::wap::type_name def) \
      const noexcept {                                                                             \
    if (WAP_UNLIKELY(bounds > size())) return def;                                                 \
    return ::wap::bits::le::type_name(slice_ + offset);                                            \
  }                                                                                                \
  inline ::wap::type_name type_name##b(::wap::u32 offset, ::wap::u32 bounds) const noexcept {      \
    if (WAP_UNLIKELY(bounds > size())) return 0;                                                   \
    return ::wap::bits::be::type_name(slice_ + offset);                                            \
  }                                                                                                \
  inline ::wap::type_name type_name##b_u(::wap::u32 offset) const noexcept {                       \
    return ::wap::bits::be::type_name(slice_ + offset);                                            \
  }                                                                                                \
  inline ::wap::type_name type_name##b(::wap::u32 offset, ::wap::u32 bounds, ::wap::type_name def) \
      const noexcept {                                                                             \
    if (WAP_UNLIKELY(bounds > size())) return def;                                                 \
    return ::wap::bits::be::type_name(slice_ + offset);                                            \
  }                                                                                                \
  inline void put_##type_name(::wap::u32 offset, ::wap::u32 bounds,                                \
                              ::wap::type_name val) noexcept {                                     \
    if (WAP_UNLIKELY(bounds > size())) return;                                                     \
    ::wap::bits::le::put_##type_name(slice_ + offset, val);                                        \
  }                                                                                                \
  inline void put_##type_name##_u(::wap::u32 offset, ::wap::type_name val) noexcept {              \
    ::wap::bits::le::put_##type_name(slice_ + offset, val);                                        \
  }                                                                                                \
  inline void put_##type_name##b(::wap::u32 offset, ::wap::u32 bounds,                             \
                                 ::wap::type_name val) noexcept {                                  \
    if (WAP_UNLIKELY(bounds > size())) return;                                                     \
    ::wap::bits::be::put_##type_name(slice_ + offset, val);                                        \
  }                                                                                                \
  inline void put_##type_name##b_u(::wap::u32 offset, ::wap::type_name val) noexcept {             \
    ::wap::bits::be::put_##type_name(slice_ + offset, val);                                        \
  }

namespace wap {
  using i8 = signed char;          // int8_t;//std::int8_t;
  using i16 = short;               // int16_t;//std::int16_t;
  using i32 = int;                 // int32_t;//std::int32_t;
  using i64 = long long;           // int64_t;//std::int64_t;
  using u8 = unsigned char;        // uint8_t;//std::uint8_t;
  using u16 = unsigned short;      // uint16_t;//std::uint16_t;
  using u32 = unsigned int;        // uint32_t;//std::uint32_t;
  using u64 = unsigned long long;  // uint64_t;//std::uint64_t;
  using f32 = float;
  using f64 = double;
  using size_t = size_t;  // size_t;//std::size_t;
  using boolean = bool;

  constexpr static const int8_t deBruijn_clz32[32]
      = {31, 22, 30, 21, 18, 10, 29, 2,  20, 17, 15, 13, 9, 6,  28, 1,
         23, 19, 11, 3,  16, 14, 7,  24, 12, 4,  8,  25, 5, 26, 27, 0};

  constexpr static inline int clz32_constexpr(uint32_t v) noexcept {
    v |= v >> 1;
    v |= v >> 2;
    v |= v >> 4;
    v |= v >> 8;
    v |= v >> 16;
    return deBruijn_clz32[v * UINT32_C(0x07C4ACDD) >> 27];
  }

  constexpr static const uint8_t deBruijn_clz64[64]
      = {63, 16, 62, 7,  15, 36, 61, 3,  6,  14, 22, 26, 35, 47, 60, 2,  9,  5,  28, 11, 13, 21,
         42, 19, 25, 31, 34, 40, 46, 52, 59, 1,  17, 8,  37, 4,  23, 27, 48, 10, 29, 12, 43, 20,
         32, 41, 53, 18, 38, 24, 49, 30, 44, 33, 54, 39, 50, 45, 55, 51, 56, 57, 58, 0};
  constexpr static inline int clz64_constexpr(uint64_t v) noexcept {
    v |= v >> 1;
    v |= v >> 2;
    v |= v >> 4;
    v |= v >> 8;
    v |= v >> 16;
    v |= v >> 32;

    return deBruijn_clz64[v * UINT64_C(0x03F79D71B4CB0A89) >> 58];
  }

#if defined(__GNUC__) || defined(__clang__)

  constexpr static inline int clz32(uint32_t v) noexcept { return __builtin_clz(v); }

  constexpr static inline int clz64(uint64_t v) noexcept { return __builtin_clzll(v); }

#elif defined(_MSC_VER)

#  pragma intrinsic(_BitScanReverse)
  constexpr static int clz32(uint32_t v) noexcept {
    if (WAP_UNLIKELY(v == 0)) return 32;
    unsigned long index;
    //    assert(v > 0);
    _BitScanReverse(&index, v);
    return (uint32_t)(31 - int(index));
  }

#  pragma intrinsic(_BitScanReverse64)
  constexpr static int clz64(uint64_t v) noexcept {
    if (WAP_UNLIKELY(v == 0)) return 64;
    unsigned long index;
    //    assert(v > 0);
    _BitScanReverse64(&index, v);
    return 63 - int(index);
  }

#else /* fallback */

  constexpr static inline int clz32(uint32_t v) noexcept { return clz32_constexpr(v); }
  constexpr static inline int clz64(uint32_t v) noexcept { return clz64_constexpr(v); }

#endif  // clz32, clz64

  struct allocator {
    constexpr static inline ::wap::u8 *allocate(const wap::size_t capacity) noexcept {
      return (wap::u8 *)malloc(capacity);
    }

    constexpr static inline ::wap::u8 *reallocate(wap::u8 *ptr, ::wap::size_t capacity) noexcept {
      return (wap::u8 *)realloc(ptr, capacity);
    }

    constexpr static inline void deallocate(wap::u8 *ptr) noexcept { return free(ptr); }

    struct noop {
      constexpr static inline ::wap::u8 *allocate(wap::size_t) { return nullptr; }

      constexpr static inline ::wap::u8 *reallocate(wap::u8 *, ::wap::size_t) noexcept {
        return nullptr;
      }

      constexpr static inline void deallocate(wap::u8 *) noexcept {}
    };
  };

  namespace bits {
#define WAP_BITS_LE_FUNCS(type_name, bits)                                        \
  inline ::wap::type_name type_name(const ::wap::u8 *ptr) {                       \
    if WAP_IS_BIG_ENDIAN                                                          \
      return (wap::type_name)bswap_##bits(*((wap::u##bits *)ptr));                \
    else                                                                          \
      return *((wap::type_name *)ptr);                                            \
  }                                                                               \
  inline void put_##type_name(const ::wap::u8 *ptr, const ::wap::type_name val) { \
    if WAP_IS_BIG_ENDIAN                                                          \
      *((wap::type_name *)ptr) = bswap_##bits((wap::u##bits)val);                 \
    else                                                                          \
      *((wap::type_name *)ptr) = val;                                             \
  }

#define WAP_BITS_BE_FUNCS(type_name, bits)                                        \
  inline ::wap::type_name type_name(const ::wap::u8 *ptr) {                       \
    if WAP_IS_BIG_ENDIAN                                                          \
      return *((wap::type_name *)ptr);                                            \
    else                                                                          \
      return (wap::type_name)bswap_##bits(*((wap::u##bits *)ptr));                \
  }                                                                               \
  inline void put_##type_name(const ::wap::u8 *ptr, const ::wap::type_name val) { \
    if WAP_IS_BIG_ENDIAN                                                          \
      *((wap::type_name *)ptr) = val;                                             \
    else                                                                          \
      *((wap::type_name *)ptr) = bswap_##bits((wap::u##bits)val);                 \
  }

    namespace le {
      WAP_BITS_LE_FUNCS(i16, 16)

      WAP_BITS_LE_FUNCS(u16, 16)

      WAP_BITS_LE_FUNCS(i32, 32)

      WAP_BITS_LE_FUNCS(u32, 32)

      WAP_BITS_LE_FUNCS(i64, 64)

      WAP_BITS_LE_FUNCS(u64, 64)

      WAP_BITS_LE_FUNCS(f32, 32)

      WAP_BITS_LE_FUNCS(f64, 64)
    }  // namespace le

    namespace be {
      WAP_BITS_BE_FUNCS(i16, 16)

      WAP_BITS_BE_FUNCS(u16, 16)

      WAP_BITS_BE_FUNCS(i32, 32)

      WAP_BITS_BE_FUNCS(u32, 32)

      WAP_BITS_BE_FUNCS(i64, 64)

      WAP_BITS_BE_FUNCS(u64, 64)

      WAP_BITS_BE_FUNCS(f32, 32)

      WAP_BITS_BE_FUNCS(f64, 64)
    }  // namespace be
  }    // namespace bits

  class vptr {
  public:
    inline ::wap::u32 offset() {
      if WAP_IS_BIG_ENDIAN
        return bswap_32(offset_);
      else
        return offset_;
    }

    inline ::wap::u32 size() {
      if WAP_IS_BIG_ENDIAN
        return bswap_32(size_);
      else
        return size_;
    }

  private:
    ::wap::u32 offset_;
    ::wap::u32 size_;
  };

  class string_vptr : public ::wap::vptr {};

  template <typename T> class list {
  public:
    list() {}

    inline i32 bytes() { return (i32)(sizeof(T) * capacity_ + sizeof(list)); }

    inline i32 length() { return length_; }

    inline i32 capacity() { return capacity_; }

  private:
    ::wap::i32 length_;
    ::wap::i32 capacity_;
  };

  template <typename T> class list_view {
  public:
    operator bool() { return list_ != nullptr; }

    i32 bytes() {
      if (list_ == nullptr) return 0;
      return list_->bytes();
    }

    i32 length() {
      if (list_ == nullptr) return 0;
      return list_->length();
    }

    i32 capacity() {
      if (list_ == nullptr) return 0;
      return list_->capacity();
    }

  private:
    list<T> *list_;
  };

  template <typename K, typename V> class map_view {
  public:
    explicit map_view() {}

  private:
  };

  class str {
  public:
    inline ::wap::u32 length() { return ::wap::bits::le::u32(data_); }

    inline ::wap::u32 capacity() { return field_.size(); }

    inline ::wap::u32 offset() { return (::wap::u32)(data_ - buffer_); }

    inline ::std::string_view get() noexcept {
      if (length() == 0) return "";
      return {(const char *)data_ + 4, static_cast<::wap::size_t>(length())};
    }

    str(u8 *buffer, vptr &field, u8 *data) : buffer_(buffer), field_(field), data_(data) {}

  private:
    ::wap::u8 *buffer_;
    ::wap::vptr &field_;
    ::wap::u8 *data_;
  };

  constexpr static const ::wap::size_t HEADER_SIZE = 16;

  template <typename Allocator = ::wap::allocator> class buffer {
  public:
    class slice {
    public:
      slice(const slice &s) : buffer_(s.buffer_), slice_(s.slice_), size_(s.size_) {}

      slice(wap::buffer<Allocator> &buffer, ::wap::u8 *slice, ::wap::size_t size)
          : buffer_(buffer), slice_(slice), size_(size) {}

      ::wap::buffer<Allocator>::slice base(wap::size_t offset, ::wap::size_t bounds) {
        if (WAP_UNLIKELY(bounds > buffer_.base_size())) {
          return {buffer_, nullptr, 0};
        }
        return {buffer_, (wap::u8 *)(slice_ + offset), buffer_.base_size()};
      }

      ::wap::buffer<Allocator>::slice flex(wap::u32 offset, ::wap::size_t size) {
        if (WAP_UNLIKELY((wap::size_t)offset + size > buffer_.size())) {
          return {buffer_, nullptr, 0};
        }
        return {buffer_, (wap::u8 *)(buffer_.buffer_ + offset), buffer_.size()};
      }

    public:
      template <typename T> T cast() { return T(*this); }

      std::string_view str(wap::size_t offset, ::wap::size_t bounds) {
        if (WAP_UNLIKELY(bounds > size_)) return "";
        ::wap::u8 first_byte = *(slice_ + offset);
        if (first_byte < 8) {
        }
        return "";
      }

      WAP_SLICE_ONE_BYTE(i8)

      WAP_SLICE_ONE_BYTE(u8)

      WAP_SLICE_ONE_BYTE(boolean)

      WAP_SLICE_ENDIANNESS(i16)

      WAP_SLICE_ENDIANNESS(u16)

      WAP_SLICE_ENDIANNESS(i32)

      WAP_SLICE_ENDIANNESS(u32)

      WAP_SLICE_ENDIANNESS(i64)

      WAP_SLICE_ENDIANNESS(u64)

      WAP_SLICE_ENDIANNESS(f32)

      WAP_SLICE_ENDIANNESS(f64)

      inline ::wap::size_t size() const noexcept { return size_; }
      inline ::wap::size_t message_size() const noexcept { return buffer_.size(); }
      inline ::wap::size_t message_base_size() const noexcept { return buffer_.base_size(); }

    private:
      ::wap::buffer<Allocator> &buffer_;
      ::wap::u8 *slice_;
      ::wap::size_t size_;
    };

    inline ::wap::size_t size() const noexcept {
      return static_cast<wap::size_t>(bits::le::u32(buffer_));
    }

    inline ::wap::size_t base_size() const noexcept {
      return (wap::size_t)bits::le::u32(buffer_ + 4);
    }

    inline ::wap::u32 capacity() const noexcept { return capacity_; }

    inline ::wap::buffer<Allocator>::slice base() noexcept {
      if (WAP_UNLIKELY(HEADER_SIZE > base_size())) return {*this, nullptr, 0};
      return {*this, (::wap::u8 *)(buffer_ + HEADER_SIZE), base_size()};
    }

    ::std::unique_ptr<::wap::u8, ::std::function<void(wap::u8 *)>> take_unique_deleter() noexcept {
      ::wap::u8 *b = buffer_;
      buffer_ = nullptr;
      return {b, [](::wap::u8 *p) { Allocator::deallocate(p); }};
    }

    ::std::unique_ptr<::wap::u8> take_unique() noexcept {
      auto b = buffer_;
      buffer_ = nullptr;
      return std::unique_ptr<u8>(b);
    }

    ::wap::u8 *take() noexcept {
      auto b = buffer_;
      buffer_ = nullptr;
      return b;
    }

    explicit operator bool() { return buffer_ != nullptr; }

    buffer(buffer &&from) noexcept {
      buffer_ = from.buffer_;
      from.buffer_ = nullptr;
      capacity_ = from.capacity_;
      from.capacity_ = 0;
    }

    buffer(const buffer &b) noexcept : buffer_(std::move(b.take())), capacity_(b.capacity_) {}

    explicit buffer(wap::u8 *buffer) noexcept : buffer_(buffer), capacity_(maybe_size()) {}

    buffer(wap::u8 *buffer, ::wap::size_t capacity) noexcept
        : buffer_(buffer), capacity_(capacity) {}

    buffer(wap::u8 *buffer, ::wap::size_t capacity, Allocator allocator) noexcept
        : buffer_(buffer), capacity_(capacity) {}

    ~buffer() {
      if (WAP_LIKELY(buffer_)) {
        Allocator::deallocate(buffer_);
        buffer_ = nullptr;
      }
    }

    //    template <typename A = ::wap::allocator>
    static ::wap::buffer<Allocator> make(wap::size_t size, ::wap::size_t base_size,
                                         ::wap::size_t capacity, ::wap::u16 type,
                                         Allocator allocator = Allocator()) {
      auto buffer = allocator.allocate(capacity);

      // TODO: fix this and replace with memcpy initializer buffer
      memset(buffer, 0, capacity);

      if (capacity < size) {
        capacity = size;
      }

      // init header
      ::wap::bits::le::put_u32(buffer, (wap::u32)size);
      ::wap::bits::le::put_u32(buffer + 4, (wap::u32)base_size);
      ::wap::bits::le::put_u64(buffer + 8, 0);
      ::wap::bits::le::put_u16(buffer + 8, type);

      return {buffer, capacity, allocator};
    }

  protected:
    ::wap::u8 *buffer_{nullptr};
    ::wap::u32 capacity_{0};

    inline ::wap::size_t maybe_size() {
      return buffer_ == nullptr ? 0 : (wap::size_t)bits::le::u32(buffer_);
    }
  };

  template <typename Msg, typename Allocator = ::wap::allocator,
            typename Reader = typename Msg::template Reader<Allocator>,
            typename Builder = typename Msg::template Builder<Allocator>,
            ::wap::i16 TypeCode = Msg::TYPE, ::wap::size_t BaseSize = Msg::BASE_SIZE,
            ::wap::size_t DefaultSize = Msg::DEFAULT_SIZE>
  class message {
  public:
    constexpr static const ::wap::size_t BASE_SIZE = BaseSize + HEADER_SIZE;

    inline Reader *operator->() noexcept { return &reader_; }

    inline ::wap::size_t size() const noexcept { return buffer_.size(); }

    inline ::wap::size_t base_size() const noexcept { return buffer_.base_size(); }

    inline ::wap::size_t capacity() const noexcept { return buffer_.capacity(); }

    inline Reader &reader() noexcept { return reader_; }
    inline Reader &get() noexcept { return reader_; }

    inline Builder builder() noexcept { return Builder(reader_); }

    explicit operator bool() { return true; }

    message(message &m) : buffer_(std::move(m.buffer_)), reader_(buffer_.base()) {}

    message &operator=(const message &m) = default;

    explicit message(wap::buffer<Allocator> buffer) noexcept
        : buffer_(std::move(buffer)), reader_(Reader(buffer_.base())) {}

    explicit message(wap::size_t extra = BaseSize) : message(Allocator(), extra) {}

    explicit message(Allocator allocator = Allocator(), ::wap::size_t extra = BaseSize)
        : buffer_(wap::buffer<Allocator>::make(BASE_SIZE, BASE_SIZE, BASE_SIZE + extra, TypeCode,
                                               allocator)),
          reader_(std::move(buffer_.base())) {}

    explicit message(wap::u8 *data) noexcept
        : buffer_(wap::buffer(data)), reader_(Reader(buffer_.base())) {}

    explicit message(void *data, ::wap::size_t capacity) noexcept
        : buffer_(wap::buffer((::wap::u8 *)data, capacity)),
          reader_(Reader(std::move(buffer_.base()))) {}

    explicit message(wap::u8 *data, ::wap::size_t capacity) noexcept
        : buffer_(wap::buffer(data, capacity)), reader_(Reader(std::move(buffer_.base()))) {}

    ~message() { printf("~message()\n"); }

  private:
    ::wap::buffer<Allocator> buffer_;
    Reader reader_;
  };

  namespace schema {}

  namespace alloc {}
}  // namespace wap

#endif  // WAP_H
