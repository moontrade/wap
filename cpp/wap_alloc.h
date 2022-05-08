#ifndef WAP_ALLOC_H
#define WAP_ALLOC_H

#include <cstdint>
#include <variant>

#include "wap.h"

#define WAP_PORTABLE_HEAP 1
#define WAP_UNSAFE_HEAP 0
#define WAP_NODISCARD [[nodiscard]]

namespace wap {
struct parse_error {
   enum reason_code {
      overflow = 1,
      malformed = 2,
   };
   const reason_code reason;
   parse_error(const reason_code r) : reason(r) {}
};
}  // namespace wap

////////////////////////////////////////////////////////////////////////////////////
// Size classes and freelist impls
////////////////////////////////////////////////////////////////////////////////////
namespace wap {
template <typename SizeT = uint32_t, typename BitmapT = uint32_t>
struct size_class {
 public:
   const uint16_t index;
   const uint16_t shift;
   const SizeT size;

   constexpr static const BitmapT ONE = (BitmapT)1U;

   constexpr size_class(uint16_t idx, uint16_t s, uint16_t sz) : index(idx), shift(s), size(sz) {}

   inline uint32_t set(BitmapT bitmap) const noexcept { return bitmap | (ONE << shift); }

   inline uint32_t unset(BitmapT bitmap) const noexcept { return bitmap & ~(ONE << shift); }

   inline int search(BitmapT bitmap) const noexcept {
      if constexpr (sizeof(BitmapT) == 8) {
         return clz64(bitmap << index) + index;
      } else {
         return clz32(bitmap << index) + index;
      }
   }
};

template <typename SizeT>
struct size_class_array {
   using SizeClassT = size_class<SizeT>;

 public:
   const SizeClassT* begin;
   const std::size_t size;
   const SizeT min;
   const SizeT max;
   const SizeT boundary;
   const SizeT div;

   template <std::size_t N>
   constexpr size_class_array(const SizeT d, const SizeT mn, const SizeClassT (&l)[N])
       : begin(l),
         size(N),
         min(mn),
         max(N > 0 ? l[N - 1].size : 0),
         boundary(max == 0 ? 0 : max + 1),
         div(d) {}

   constexpr size_class_array(const size_class_array<SizeT>& l) = default;

   constexpr static SizeT div_round_up(SizeT n, SizeT a) noexcept {
      return (SizeT)(((int)n + (int)a - 1) / (int)a);
   }

   WAP_NODISCARD constexpr SizeClassT get(SizeT sz) const noexcept {
      const SizeT index = div_round_up(sz - min, div);
      return begin[index];
   }

   constexpr SizeClassT operator[](int index) const noexcept { return begin[index]; }
};

template <typename SizeT, size_class_array<SizeT> Classes, size_class_array<SizeT> Small,
   size_class_array<SizeT> Medium, size_class_array<SizeT> Large>
struct size_classes {
   using SizeClassT = size_class<SizeT>;

   constexpr static const SizeT MIN_SPLIT = Small.min + sizeof(uint32_t);

 public:
   constexpr static inline SizeClassT alloc_size(SizeT size) {
      if (WAP_LIKELY(size < Small.boundary)) {
         return Small.get(size);
      }
      if constexpr (Medium.div > 0) {
         if (WAP_LIKELY(size < Medium.boundary)) {
            return Medium.get(size);
         }
      }
      if constexpr (Large.div > 0) {
         if (WAP_LIKELY(size < Large.boundary)) {
            return Large.get(size);
         }
      }
      return {0, 0, 0};
   }

   constexpr static inline SizeClassT pool_size(SizeT size) {
      auto cls = alloc_size(size);
      if (WAP_UNLIKELY(size >= cls.size)) {
         return cls;
      }
      if (WAP_UNLIKELY(cls.index == 0)) return {0, 0, 0};
      return Classes[cls.index - 1];
   }

   constexpr static inline int class_count() { return Classes.size; }
   constexpr static inline SizeClassT get_class(int index) { return Classes[index]; }
   constexpr static inline SizeT get_min_size() { return Classes.min; }
   constexpr static inline SizeT get_max_size() { return Classes.max; }
   constexpr static inline SizeT get_min_split() { return MIN_SPLIT; }
   constexpr static inline bool can_split(SizeT current, SizeT size) {
      return current - size >= Classes.min;
   }
};

template <typename SizeT, typename Classes>
class free_list {
 public:
   using BitmapT = uint32_t;
   using SizeClassT = size_class<SizeT>;
   using ListSizeT = SizeT;
   constexpr static const SizeT CLASS_COUNT = Classes::class_count();
   constexpr static SizeClassT get_class(int index) { return Classes::get_class(index); }

   free_list() { clear(); }

 protected:
   BitmapT fl_{};
   SizeT freelist_[CLASS_COUNT]{};

 public:
   constexpr static inline SizeClassT alloc_size(SizeT size) { return Classes::alloc_size(size); }
   constexpr static inline SizeClassT pool_size(SizeT size) { return Classes::pool_size(size); }
   constexpr static inline SizeT get_min_size() { return Classes::get_min_size(); }
   constexpr static inline SizeT get_max_size() { return Classes::get_max_size(); }
   constexpr static inline SizeT get_min_split() { return Classes::get_min_split(); }
   constexpr static inline bool can_split(SizeT current, SizeT size) {
      return Classes::can_split(current, size);
   }

   inline void clear() { memset(this, 0, sizeof(*this)); }

   inline void insert(SizeClassT cls, SizeT value) {
      fl_ = cls.set(fl_);
      freelist_[cls.index] = value;
   }

   inline void insert_size(SizeT size, SizeT value) {
      size_class cls = alloc_size(size);
      if (cls.size == 0) {
         return;
      }
      fl_ = cls.set(fl_);
      freelist_[cls.index] = value;
   }

   inline void remove(SizeClassT cls) {
      fl_ = cls.unset(fl_);
      freelist_[cls.index] = 0;
   }

   inline SizeT get(SizeClassT cls) { return freelist_[cls.index]; }

   inline void set(SizeClassT cls, SizeT offset) {
      fl_ = cls.set(fl_);
      freelist_[cls.index] = offset;
   }

   inline SizeT search(SizeT size) {
      size_class cls = alloc_size(size);
      if (WAP_UNLIKELY(cls.size == 0)) return 0;
      const int index = cls.search(fl_);
      if (WAP_LIKELY(index < CLASS_COUNT)) return freelist_[index];
      return 0;
   }

   inline SizeT search(SizeClassT cls) {
      const int index = cls.search(fl_);
      if (WAP_LIKELY(index < CLASS_COUNT)) return freelist_[index];
      return 0;
   }

   inline SizeClassT search_class(SizeClassT cls) {
      const int index = cls.search(fl_);
      if (WAP_LIKELY(index < CLASS_COUNT)) return get_class(index);
      return {0, 0, 0};
   }

   inline SizeClassT search_class(SizeT size) {
      size_class cls = alloc_size(size);
      if (WAP_UNLIKELY(cls.size == 0)) return {0, 0, 0};
      const int index = cls.search(fl_);
      if (WAP_LIKELY(index < CLASS_COUNT)) return get_class(index);
      return {0, 0, 0};
   }
};

// 16bit freelist
class small {
 private:
   class data {
    private:
      constexpr static size_class<uint16_t> sizes[] = {{0, 31, 12}, {1, 30, 16}, {2, 29, 20},
         {3, 28, 24}, {4, 27, 28}, {5, 26, 36}, {6, 25, 44}, {7, 24, 60}, {8, 23, 76}, {9, 22, 92},
         {10, 21, 124}, {11, 20, 156}, {12, 19, 188}, {13, 18, 220}, {14, 17, 252}, {15, 16, 300},
         {16, 15, 364}, {17, 14, 428}, {18, 13, 508}, {19, 12, 764}, {20, 11, 1020}, {21, 10, 1280},
         {22, 9, 1532}, {23, 8, 2044}, {24, 7, 4092}, {25, 6, 8188}, {26, 5, 16380}};
      constexpr static size_class<uint16_t> small[]{{0, 31, 12}, {0, 31, 12}, {0, 31, 12},
         {0, 31, 12}, {1, 30, 16}, {2, 29, 20}, {3, 28, 24}, {4, 27, 28}, {5, 26, 36}, {5, 26, 36},
         {6, 25, 44}, {6, 25, 44}, {7, 24, 60}, {7, 24, 60}, {7, 24, 60}, {7, 24, 60}, {8, 23, 76},
         {8, 23, 76}, {8, 23, 76}, {8, 23, 76}, {9, 22, 92}, {9, 22, 92}, {9, 22, 92}, {9, 22, 92},
         {10, 21, 124}, {10, 21, 124}, {10, 21, 124}, {10, 21, 124}, {10, 21, 124}, {10, 21, 124},
         {10, 21, 124}, {10, 21, 124}, {11, 20, 156}, {11, 20, 156}, {11, 20, 156}, {11, 20, 156},
         {11, 20, 156}, {11, 20, 156}, {11, 20, 156}, {11, 20, 156}, {12, 19, 188}, {12, 19, 188},
         {12, 19, 188}, {12, 19, 188}, {12, 19, 188}, {12, 19, 188}, {12, 19, 188}, {12, 19, 188},
         {13, 18, 220}, {13, 18, 220}, {13, 18, 220}, {13, 18, 220}, {13, 18, 220}, {13, 18, 220},
         {13, 18, 220}, {13, 18, 220}, {14, 17, 252}, {14, 17, 252}, {14, 17, 252}, {14, 17, 252},
         {14, 17, 252}, {14, 17, 252}, {14, 17, 252}, {14, 17, 252}, {15, 16, 300}, {15, 16, 300},
         {15, 16, 300}, {15, 16, 300}, {15, 16, 300}, {15, 16, 300}, {15, 16, 300}, {15, 16, 300},
         {15, 16, 300}, {15, 16, 300}, {15, 16, 300}, {15, 16, 300}, {16, 15, 364}, {16, 15, 364},
         {16, 15, 364}, {16, 15, 364}, {16, 15, 364}, {16, 15, 364}, {16, 15, 364}, {16, 15, 364},
         {16, 15, 364}, {16, 15, 364}, {16, 15, 364}, {16, 15, 364}, {16, 15, 364}, {16, 15, 364},
         {16, 15, 364}, {16, 15, 364}, {17, 14, 428}, {17, 14, 428}, {17, 14, 428}, {17, 14, 428},
         {17, 14, 428}, {17, 14, 428}, {17, 14, 428}, {17, 14, 428}, {17, 14, 428}, {17, 14, 428},
         {17, 14, 428}, {17, 14, 428}, {17, 14, 428}, {17, 14, 428}, {17, 14, 428}, {17, 14, 428},
         {18, 13, 508}, {18, 13, 508}, {18, 13, 508}, {18, 13, 508}, {18, 13, 508}, {18, 13, 508},
         {18, 13, 508}, {18, 13, 508}, {18, 13, 508}, {18, 13, 508}, {18, 13, 508}, {18, 13, 508},
         {18, 13, 508}, {18, 13, 508}, {18, 13, 508}, {18, 13, 508}, {18, 13, 508}, {18, 13, 508},
         {18, 13, 508}};
      constexpr static size_class<uint16_t> medium[]{{18, 13, 508}, {19, 12, 764}, {20, 11, 1020},
         {21, 10, 1280}, {22, 9, 1532}, {23, 8, 2044}, {23, 8, 2044}, {24, 7, 4092}, {24, 7, 4092},
         {24, 7, 4092}, {24, 7, 4092}, {24, 7, 4092}, {24, 7, 4092}, {24, 7, 4092}, {24, 7, 4092},
         {25, 6, 8188}, {25, 6, 8188}, {25, 6, 8188}, {25, 6, 8188}, {25, 6, 8188}, {25, 6, 8188},
         {25, 6, 8188}, {25, 6, 8188}, {25, 6, 8188}, {25, 6, 8188}, {25, 6, 8188}, {25, 6, 8188},
         {25, 6, 8188}, {25, 6, 8188}, {25, 6, 8188}, {25, 6, 8188}, {26, 5, 16380}, {26, 5, 16380},
         {26, 5, 16380}, {26, 5, 16380}, {26, 5, 16380}, {26, 5, 16380}, {26, 5, 16380},
         {26, 5, 16380}, {26, 5, 16380}, {26, 5, 16380}, {26, 5, 16380}, {26, 5, 16380},
         {26, 5, 16380}, {26, 5, 16380}, {26, 5, 16380}, {26, 5, 16380}, {26, 5, 16380},
         {26, 5, 16380}, {26, 5, 16380}, {26, 5, 16380}, {26, 5, 16380}, {26, 5, 16380},
         {26, 5, 16380}, {26, 5, 16380}, {26, 5, 16380}, {26, 5, 16380}, {26, 5, 16380},
         {26, 5, 16380}, {26, 5, 16380}, {26, 5, 16380}, {26, 5, 16380}};
      constexpr static size_class<uint16_t> large[]{{0, 0, 0}};

    public:
      constexpr static size_class_array<uint16_t> SIZES = {0, 0, sizes};
      constexpr static size_class_array<uint16_t> SMALL = {4, 0, small};
      constexpr static size_class_array<uint16_t> MEDIUM = {256, SMALL.max, medium};
      constexpr static size_class_array<uint16_t> LARGE = {0, 0, large};
   };

 public:
   using sizes = size_classes<uint16_t, data::SIZES, data::SMALL, data::MEDIUM, data::LARGE>;

   struct free_list : public ::wap::free_list<uint16_t, sizes> {
    public:
      free_list() = default;
   };
};
}  // namespace wap

#define WAP_SLICE_BIT_FUNCS(type_name)                                              \
   WAP_NODISCARD ::wap::type_name type_name(wap::size_t offset) const noexcept {    \
      return ::wap::bits::le::type_name(voffset_of(offset));                        \
   }                                                                                \
   void type_name(wap::size_t offset, ::wap::type_name value) noexcept {            \
      ::wap::bits::le::put_##type_name(voffset_of(offset), value);                  \
   }                                                                                \
   WAP_NODISCARD ::wap::type_name type_name##b(wap::size_t offset) const noexcept { \
      return ::wap::bits::be::type_name(voffset_of(offset));                        \
   }                                                                                \
   void type_name##b(wap::size_t offset, ::wap::type_name value) noexcept {         \
      ::wap::bits::be::put_##type_name(voffset_of(offset), value);                  \
   }                                                                                \
   WAP_NODISCARD ::wap::type_name type_name##n(wap::size_t offset) const noexcept { \
      return *(::wap::type_name*)(voffset_of(offset));                              \
   }                                                                                \
   void type_name##n(wap::size_t offset, ::wap::type_name value) noexcept {         \
      *(::wap::type_name*)(voffset_of(offset)) = value;                             \
   }

#define WAP_SLICE_FUNCS                                            \
   WAP_NODISCARD ::wap::i8 i8(wap::size_t offset) const noexcept { \
      return *(::wap::i8*)(voffset_of(offset));                    \
   }                                                               \
   void i8(wap::size_t offset, ::wap::i8 value) const noexcept {   \
      *(::wap::i8*)(voffset_of(offset)) = value;                   \
   }                                                               \
   WAP_NODISCARD ::wap::u8 u8(wap::size_t offset) const noexcept { \
      return *(::wap::u8*)(voffset_of(offset));                    \
   }                                                               \
   void u8(wap::size_t offset, ::wap::u8 value) const noexcept {   \
      *(::wap::u8*)(voffset_of(offset)) = value;                   \
   }                                                               \
   WAP_SLICE_BIT_FUNCS(i16)                                        \
   WAP_SLICE_BIT_FUNCS(i32)                                        \
   WAP_SLICE_BIT_FUNCS(i64)                                        \
   WAP_SLICE_BIT_FUNCS(u16)                                        \
   WAP_SLICE_BIT_FUNCS(u32)                                        \
   WAP_SLICE_BIT_FUNCS(u64)                                        \
   WAP_SLICE_BIT_FUNCS(f32)                                        \
   WAP_SLICE_BIT_FUNCS(f64)

#define WAP_SLICE_BIT_BOUNDS_CHECK_FUNCS(type_name)                                           \
   WAP_NODISCARD ::wap::type_name type_name(wap::size_t offset) const noexcept {              \
      if (WAP_UNLIKELY(voffset_of(offset) + sizeof(::wap::type_name) > base_end())) return 0; \
      return ::wap::bits::le::type_name(voffset_of(offset));                                  \
   }                                                                                          \
   void type_name(wap::size_t offset, ::wap::type_name value) noexcept {                      \
      if (WAP_UNLIKELY(voffset_of(offset) + sizeof(::wap::type_name) > base_end())) return;   \
      ::wap::bits::le::put_##type_name(voffset_of(offset), value);                            \
   }                                                                                          \
   WAP_NODISCARD ::wap::type_name type_name##b(wap::size_t offset) const noexcept {           \
      if (WAP_UNLIKELY(voffset_of(offset) + sizeof(::wap::type_name) > base_end())) return 0; \
      return ::wap::bits::be::type_name(voffset_of(offset));                                  \
   }                                                                                          \
   void type_name##b(wap::size_t offset, ::wap::type_name value) noexcept {                   \
      if (WAP_UNLIKELY(voffset_of(offset) + sizeof(::wap::type_name) > base_end())) return;   \
      ::wap::bits::be::put_##type_name(voffset_of(offset), value);                            \
   }                                                                                          \
   WAP_NODISCARD ::wap::type_name type_name##n(wap::size_t offset) const noexcept {           \
      if (WAP_UNLIKELY(voffset_of(offset) + sizeof(::wap::type_name) > base_end())) return 0; \
      return *(::wap::type_name*)(voffset_of(offset));                                        \
   }                                                                                          \
   void type_name##n(wap::size_t offset, ::wap::type_name value) noexcept {                   \
      if (WAP_UNLIKELY(voffset_of(offset) + sizeof(::wap::type_name) > base_end())) return;   \
      *(::wap::type_name*)(voffset_of(offset)) = value;                                       \
   }

#define WAP_SLICE_SAFE_FUNCS                                        \
   WAP_NODISCARD ::wap::i8 i8(wap::size_t offset) const noexcept {  \
      if (WAP_UNLIKELY(voffset_of(offset) >= base_end())) return 0; \
      return *(::wap::i8*)(voffset_of(offset));                     \
   }                                                                \
   void i8(wap::size_t offset, ::wap::i8 value) const noexcept {    \
      if (WAP_UNLIKELY(voffset_of(offset) >= base_end())) return;   \
      *(::wap::i8*)(voffset_of(offset)) = value;                    \
   }                                                                \
   WAP_NODISCARD ::wap::u8 u8(wap::size_t offset) const noexcept {  \
      if (WAP_UNLIKELY(voffset_of(offset) >= base_end())) return 0; \
      return *(::wap::u8*)(voffset_of(offset));                     \
   }                                                                \
   void u8(wap::size_t offset, ::wap::u8 value) const noexcept {    \
      if (WAP_UNLIKELY(voffset_of(offset) >= base_end())) return;   \
      *(::wap::u8*)(voffset_of(offset)) = value;                    \
   }                                                                \
   WAP_SLICE_BIT_BOUNDS_CHECK_FUNCS(i16)                            \
   WAP_SLICE_BIT_BOUNDS_CHECK_FUNCS(i32)                            \
   WAP_SLICE_BIT_BOUNDS_CHECK_FUNCS(i64)                            \
   WAP_SLICE_BIT_BOUNDS_CHECK_FUNCS(u16)                            \
   WAP_SLICE_BIT_BOUNDS_CHECK_FUNCS(u32)                            \
   WAP_SLICE_BIT_BOUNDS_CHECK_FUNCS(u64)                            \
   WAP_SLICE_BIT_BOUNDS_CHECK_FUNCS(f32)                            \
   WAP_SLICE_BIT_BOUNDS_CHECK_FUNCS(f64)

namespace wap {
template <typename... Ts>
struct match : Ts... {
   using Ts::operator()...;
};
template <class... Ts>
match(Ts...) -> match<Ts...>;

template <typename R, typename E>
using result = std::variant<R, E>;


template <typename SizeT = wap::u16>
struct vpointer_t {
   SizeT offset;
};

// strings have an optional inlined performance feature that allows the data to be inlined
// up to the specified InlineMax - sizeof(SizeT).
template <typename SizeT = wap::u16, wap::size_t InlineMax = 8>
struct string_ptr_t {
   constexpr static const SizeT BLOCK_BIT = 1 << 0;
   SizeT offset;
};

using voffset_t = uint32_t;
using block_size_t = uint32_t;

struct grow {
   struct doubled {
      constexpr static uint32_t calc(uint32_t current, uint32_t min) noexcept {
         return WAP_MAX(current * 2, current + min);
      }
   };

   struct minimal {
      constexpr static uint32_t calc(uint32_t current, uint32_t min) noexcept {
         return current + min;
      }
   };

   template <uint32_t Min>
   struct at_least {
      constexpr static uint32_t calc(uint32_t current, uint32_t min) noexcept {
         return WAP_MAX(current + Min, current + min);
      }
   };
};

struct alloc_result {
   wap::u8* ptr;
   wap::size_t offset;
   wap::size_t size;
};

struct construct_result {
   wap::u8* data;
   wap::u8* base;
   wap::u8* tail;
   wap::u8* end;

   construct_result(u8* data, u8* base, u8* tail, u8* anEnd)
       : data(data), base(base), tail(tail), end(anEnd) {}
};

template <typename M, typename SizeT = typename M::SizeT>
struct header {
   constexpr static const auto SIZE = sizeof(SizeT) * 2;
   constexpr static const auto SIZE_OFFSET = 0;
   constexpr static const auto BASE_VOFFSET = sizeof(SizeT);
   constexpr static const auto BASE_BLOCK_OFFSET = SIZE - BASE_VOFFSET;

   constexpr static inline wap::size_t size(wap::u8* buf) noexcept {
      if constexpr (sizeof(SizeT) == 2) {
         return (size_t)wap::bits::le::u16(buf);
      } else if constexpr (sizeof(SizeT) == 4) {
         return (size_t)wap::bits::le::u32(buf);
      } else if constexpr (sizeof(SizeT) == 8) {
         return (size_t)wap::bits::le::u64(buf);
      } else {
         return 0;
      }
   }

   constexpr static inline void set_message_size(wap::u8* buf, wap::size_t size) noexcept {
      if constexpr (sizeof(SizeT) == 2) {
         wap::bits::le::put_u16(buf, (wap::u16)size);
      } else if constexpr (sizeof(SizeT) == 4) {
         wap::bits::le::put_u32(buf, (wap::u32)size);
      } else if constexpr (sizeof(SizeT) == 8) {
         wap::bits::le::put_u64(buf, (wap::u64)size);
      }
   }

   constexpr static inline void set_base_size(wap::u8* buf, wap::size_t size) noexcept {
      if constexpr (sizeof(SizeT) == 2) {
         wap::bits::le::put_u16(buf + BASE_BLOCK_OFFSET, (wap::u16)size);
      } else if constexpr (sizeof(SizeT) == 4) {
         wap::bits::le::put_u32(buf + BASE_BLOCK_OFFSET, (wap::u32)size);
      } else if constexpr (sizeof(SizeT) == 8) {
         wap::bits::le::put_u64(buf + BASE_BLOCK_OFFSET, (wap::u64)size);
      }
   }

   constexpr static inline void set_size(wap::u8* buf, wap::size_t size) noexcept {
      if constexpr (sizeof(SizeT) == 2) {
         wap::bits::le::put_u16(buf, (wap::u16)size);
      } else if constexpr (sizeof(SizeT) == 4) {
         wap::bits::le::put_u32(buf, (wap::u32)size);
      } else if constexpr (sizeof(SizeT) == 8) {
         wap::bits::le::put_u64(buf, (wap::u64)size);
      }
   }

   constexpr static inline wap::size_t base_size(wap::u8* buf) noexcept {
      if constexpr (sizeof(SizeT) == 2) {
         return (wap::size_t)wap::bits::le::u16(buf + sizeof(SizeT));
      } else if constexpr (sizeof(SizeT) == 4) {
         return (wap::size_t)wap::bits::le::u32(buf + sizeof(SizeT));
      } else if constexpr (sizeof(SizeT) == 8) {
         return (wap::size_t)wap::bits::le::u64(buf + sizeof(SizeT));
      } else {
         return 0;
      }
   }

   constexpr static inline wap::u8* base(wap::u8* buf) noexcept { return buf + SIZE; }

   constexpr static inline wap::u8* base_block(wap::u8* buf) noexcept {
      return buf + BASE_BLOCK_OFFSET;
   }

   constexpr static inline wap::u8* end(wap::u8* buf) noexcept { return buf + size(buf); }

   constexpr static inline wap::u8* offset(wap::u8* buf, wap::size_t offset) {
      return buf + offset;
   }
};

template <typename M, typename SizeT = typename M::SizeT, typename H = header<M, SizeT>,
   typename A = wap::allocator>
class msg {
 public:
   using Model = M;
   using Allocator = A;
   using Header = H;

   constexpr static const wap::size_t BASE_SIZE = Model::BASE_SIZE;
   constexpr static const wap::size_t BASE_SIZE_PLUS_SIZE = Model::BASE_SIZE + sizeof(SizeT);
   constexpr static const wap::size_t MINIMAL_CONSTRUCT_SIZE = Header::SIZE + Model::BASE_SIZE;

   struct block {
    private:
      SizeT size;
      SizeT prev_free;
      SizeT next_free;

    public:
      // User data starts directly after the size field in a used block.

      // Since block sizes are always at least a multiple of 4, the two least
      // significant bits of the size field are used to store the block status:
      // - bit 0: whether block is used or free
      // - bit 1: whether previous block is used or free
      constexpr static const SizeT FREE_BIT = 1 << 0;
      constexpr static const SizeT PREV_FREE_BIT = 1 << 1;

      // The size of the block header exposed to used blocks is the size field.
      // The prev_phys field is stored *inside* the previous free block.
      constexpr static const SizeT OVERHEAD = sizeof(SizeT);
      constexpr static const SizeT DATA_OFFSET = sizeof(SizeT) * 2;

      /* User data starts directly after the size field in a used block. */

      WAP_NODISCARD inline SizeT get_size() const { return size & ~(FREE_BIT | PREV_FREE_BIT); }

      inline void set_size(SizeT value) { size = value | (size & (FREE_BIT | PREV_FREE_BIT)); }

      WAP_NODISCARD inline bool is_free() const { return size & FREE_BIT; }

      inline void set_free() { size |= FREE_BIT; }

      inline void set_used() noexcept { size &= ~FREE_BIT; }

      WAP_NODISCARD inline bool is_prev_free() const noexcept { return size & PREV_FREE_BIT; }

      WAP_NODISCARD inline SizeT get_prev_phys() const noexcept {
         if (is_prev_free()) return get_prev_phys_unsafe();
         return 0;
      }

      WAP_NODISCARD inline SizeT get_prev_phys_unsafe() const noexcept {
         return (SizeT)WAP_POINTER_OFFSET(this, -sizeof(SizeT));
      }

      inline void set_prev_phys_unsafe(SizeT value) {
         *(SizeT*)WAP_POINTER_OFFSET(this, -sizeof(SizeT)) = value;
      }

      inline void set_prev_free() noexcept { size |= PREV_FREE_BIT; }

      inline void set_prev_used() noexcept { size &= ~PREV_FREE_BIT; }
   };

   class walker {
    public:
      using callback_fn = void(wap::u8* ptr, uint32_t block_offset, uint32_t ptr_offset,
         uint32_t size, bool used, void* user);

      constexpr static void print_walker(uint8_t* ptr, uint32_t block_offset, uint32_t ptr_offset,
         uint32_t size, bool used, void* user) {
         (void)user;
         printf("[%s]  offset: %u  size: %u \t(%p)\n", used ? "used" : "free", (uint32_t)ptr_offset,
            (uint32_t)size, (void*)block::from_ptr(ptr));
      }

      static void print(uint8_t* buffer, void* user = nullptr) {
         walk(print_walker, buffer, user);
         printf("\n");
      }

      static void walk(callback_fn walker, uint8_t* buffer, void* user) {
         auto total_size = Header::size(buffer);

         auto block = (msg::block*)buffer;
         auto offset = (uint32_t)WAP_POINTER_DIFF(block, buffer);

         while (block) {
            auto const size = block->get_size();
            auto const is_free = block->is_free();

            walker(block->to_ptr(), offset, offset + msg::block::OVERHEAD, size, !is_free, user);

            offset += size + msg::block::OVERHEAD;

            if (offset >= total_size) {
               break;
            }

            block = (msg::block*)(buffer + offset);
         }
      }
   };

   ////////////////////////////////////////////////////////////////////////////////
   // root holder w/ base mixin
   ////////////////////////////////////////////////////////////////////////////////

   // root owns the underlying message buffer.
   template <typename Base, typename Allocator = A>
   class root : public Base {
    public:
      inline wap::u8* data() const noexcept { return data_; }

      inline size_t size() const noexcept { return Header::size(data_); }

      inline size_t base_size() const noexcept { return Header::base_size(data_); }

      inline wap::u8* end() const noexcept { return data_ + size(); }

      inline size_t header_size() const noexcept { return Header::SIZE; }

      inline wap::u8* base_block() noexcept { return Header::base_block(data_); }

      inline wap::u8* voffset_of(wap::size_t offset) const noexcept {
         return Header::base_block(data_) + offset;
      }

      root() = delete;
      root(root& r) = delete;
      root(root&& r) noexcept {
         memcpy(this, r.self(), sizeof(*this));
         memset(r.self(), 0, sizeof(*this));
      }
      root& operator=(root&& r) noexcept {
         memcpy(this, r.self(), sizeof(*this));
         memset(r.self(), 0, sizeof(*this));
         return *this;
      }
      ~root() {
         if (data_) {
            Allocator::deallocate(data_);
            data_ = nullptr;
         }
      }

    protected:
      template <class... Args>
      explicit root(wap::u8* data, Args&&... args)
          : Base(std::forward<Args>(args)...), data_(data) {}

      inline void clear() { memset(this, 0, sizeof(*this)); }

      inline root* self() const noexcept { return this; }

      wap::u8* data_;
   };

   struct trusted_accessors {
      WAP_SLICE_FUNCS

      inline wap::u8* base() const noexcept { return base_; }
      inline wap::u8* voffset_of(wap::size_t offset) const noexcept { return base_ + offset; }

      trusted_accessors(wap::u8* base) : base_(base) {}

      wap::u8* base_;
   };

   template <typename R>
   struct trusted_accessors_builder {
      WAP_SLICE_FUNCS

      // Builders may reallocate the underlying message buffer so we must
      // not hold a slice buffer. Always offset from the root's base.
      inline wap::u8* base() const noexcept { return r_.voffset_of(offset_); }

      inline wap::u8* voffset_of(wap::size_t offset) const noexcept {
         return r_.voffset_of(offset_ + offset);
      }

      trusted_accessors_builder(R& r, wap::size_t offset) : r_(r), offset_(offset) {}

      inline R& root() { return r_; }

      R& r_;
      const wap::size_t offset_;
   };

   struct trusted_accessors_mutable {
      WAP_SLICE_FUNCS

      // Builders may reallocate the underlying message buffer so we must
      // not hold a slice buffer. Always offset from the root's base.
      inline wap::u8* base() const noexcept { return *data_ + offset_; }

      inline wap::u8* voffset_of(wap::size_t offset) const noexcept {
         return *data_ + offset_ + offset;
      }

      trusted_accessors_mutable(wap::u8** data, wap::size_t offset)
          : data_(data), offset_(offset) {}

      wap::u8** data_;
      const wap::size_t offset_;
   };

   struct untrusted_accessors {
      WAP_SLICE_SAFE_FUNCS

      inline wap::u8* base() const noexcept { return base_; }
      inline wap::u8* base_end() const noexcept { return base_end_; }
      inline wap::u8* voffset_of(wap::size_t offset) const noexcept { return base_ + offset; }

      untrusted_accessors(wap::u8* base, wap::u8* base_end) : base_(base), base_end_(base_end) {}

      wap::u8* base_;
      wap::u8* base_end_;
   };

   template <typename R>
   struct partial_trusted_slice;
   template <typename R>
   struct trusted_slice;
   template <typename R>
   struct untrusted_slice;

   // Fixed and Inner are NOT bounds checked.
   // VPointers are NOT bounds checked.
   template <typename R>
   struct trusted_block : public trusted_accessors {
      using Fixed = trusted_accessors;
      using Inner = trusted_slice<R>;
      using Block = trusted_block<R>;

      inline R& root() const { return root_; }

      inline block* as_block() const { return (block*)this->base_; }

      inline SizeT size() const { return as_block()->get_size(); }

      inline wap::u8* data() { return this->base_ + sizeof(SizeT); }

      std::string_view get() {
         return std::string_view((const char*)this->base_ + block::OVERHEAD, size());
      }

      Inner inner(wap::size_t offset, wap::size_t size) {
         return Inner(root_, this->base_ + offset);
      }

      Fixed fixed(wap::size_t offset, wap::size_t size) const { return Fixed(this->base_ + offset); }

      inline std::string_view string(wap::size_t offset) const { return root_.string(offset); }

      inline std::optional<std::string_view> optional_string(wap::size_t offset) {
         return root_.optional_string(offset);
      }

      inline std::optional<Block> optional_block(wap::size_t offset) {
         return root_.optional_block(offset);
      }

      trusted_block(R& root, ::wap::u8* base) : trusted_accessors(base), root_(root) {}

    protected:
      R& root_;
   };

   // Fixed and Inner are NOT bounds checked.
   // VPointers are bounds checked.
   template <typename R>
   struct partial_trusted_block : public trusted_accessors {
      using Fixed = trusted_accessors;
      using Inner = partial_trusted_slice<R>;
      using Block = partial_trusted_block<R>;

      inline R& root() const { return root_; }

      inline block* as_block() { return (block*)this->base_; }
      inline SizeT size() { return as_block()->get_size(); }

      inline wap::u8* data() { return this->base_ + sizeof(SizeT); }

      std::string_view get() {
         return std::string_view((const char*)this->base_ + block::OVERHEAD, size());
      }

      Inner inner(wap::size_t offset, wap::size_t size) {
         return Inner(root_, this->base_ + offset);
      }

      Fixed fixed(wap::size_t offset, wap::size_t size) const { return Fixed(this->base_ + offset); }

      inline std::string_view string(wap::size_t offset) const { return root_.string(offset); }

      inline std::optional<std::string_view> optional_string(wap::size_t offset) {
         return root_.optional_string(offset);
      }

      inline std::optional<Block> optional_block(wap::size_t offset) {
         return root_.optional_block(offset);
      }

      partial_trusted_block(R& root, ::wap::u8* base) : trusted_accessors(base), root_(root) {}

    protected:
      R& root_;
   };

   // Fixed and Inner are bounds checked.
   // VPointers are bounds checked.
   template <typename R>
   struct untrusted_block : public untrusted_accessors {
      using Fixed = untrusted_accessors;
      using Inner = untrusted_slice<R>;
      using Block = untrusted_block<R>;

      inline R& root() { return root_; }

      inline block* as_block() { return (block*)this->base_; }
      inline SizeT size() { return as_block()->get_size(); }

      inline wap::u8* data() { return this->base_ + sizeof(SizeT); }

      std::string_view get() {
         return std::string_view((const char*)this->base_ + block::OVERHEAD, size());
      }

      Inner inner(wap::size_t offset, wap::size_t size) {
         return Inner(root_, this->base_ + offset);
      }

      Fixed fixed(wap::size_t offset, wap::size_t size) { return Fixed(this->base_ + offset); }

      inline std::string_view string(wap::size_t offset) { return root_.string(offset); }

      inline std::optional<std::string_view> optional_string(wap::size_t offset) {
         return root_.optional_string(offset);
      }

      inline std::optional<Block> optional_block(wap::size_t offset) {
         return root_.optional_block(offset);
      }

      untrusted_block(R& root, ::wap::u8* base, wap::u8* base_end)
          : untrusted_accessors(base, base_end), root_(root) {}

    protected:
      R& root_;
   };

   // Fixed and Inner are NOT bounds checked.
   // VPointers are NOT bounds checked.
   template <typename R>
   struct trusted_slice : public trusted_accessors {
      using Fixed = trusted_accessors;
      using Inner = trusted_slice<R>;
      using Block = trusted_block<R>;

      inline R& root() { return root_; }

      Fixed fixed(wap::size_t offset, wap::size_t size) const { return Fixed(this->base_ + offset); }

      Inner inner(wap::size_t offset) const { return Inner(root_, this->base() + offset); }

      inline std::string_view string(wap::size_t offset) { return root_.string(offset); }

      inline std::optional<std::string_view> optional_string(wap::size_t offset) {
         return root_.optional_string(offset);
      }

      inline std::optional<Block> optional_block(wap::size_t offset) {
         return root_.optional_block(offset);
      }

      trusted_slice(R& root, ::wap::u8* base) : trusted_accessors(base), root_(root) {}

    protected:
      R& root_;
   };

   // Fixed and Inner are NOT bounds checked.
   // VPointers are bounds checked.
   template <typename R>
   struct partial_trusted_slice : public trusted_accessors {
      using Fixed = trusted_accessors;
      using Inner = partial_trusted_slice<R>;
      using Block = partial_trusted_block<R>;

      inline R& root() { return root_; }

      Fixed fixed(wap::size_t offset, wap::size_t size) const { return Fixed(this->base_ + offset); }

      Inner inner(wap::size_t offset) const { return Inner(root_, this->base() + offset); }

      inline std::string_view string(wap::size_t offset) { return root_.string(offset); }

      inline std::optional<std::string_view> optional_string(wap::size_t offset) {
         return root_.optional_string(offset);
      }

      inline std::optional<Block> optional_block(wap::size_t offset) {
         return root_.optional_block(offset);
      }

      partial_trusted_slice(R& root, ::wap::u8* base) : trusted_accessors(base), root_(root) {}

    protected:
      R& root_;
   };

   // Fixed and Inner are bounds checked.
   // VPointers are bounds checked.
   template <typename R>
   struct untrusted_slice : untrusted_accessors {
      using Fixed = untrusted_accessors;
      using Inner = untrusted_slice<R>;
      using Block = partial_trusted_block<R>;

      inline R& root() { return root_; }

      Inner inner(wap::size_t offset, wap::size_t size) const {
         return Inner(root_, this->base_ + offset, this->base_ + offset + size);
      }

      inline std::string_view string(wap::size_t offset) { return root_.string(offset); }

      inline std::optional<std::string_view> optional_string(wap::size_t offset) {
         return root_.optional_string(offset);
      }

      inline std::optional<Block> optional_block(wap::size_t offset) {
         return root_.optional_block(offset);
      }

      untrusted_slice(R& root, ::wap::u8* base, ::wap::u8* base_end)
          : untrusted_accessors(base, base_end), root_(root) {}

    protected:
      R& root_;
   };

#define WAP_VIEW_CTORS(name, slice_name, ...)                                        \
   name(name& r) noexcept = delete;                                                  \
   name(name&& r) noexcept : root<slice_name, Allocator>(__VA_ARGS__) { r.clear(); } \
   name& operator=(name& r) noexcept = delete;                                       \
   name& operator=(name&& r) noexcept {                                              \
      memcpy(this, r.self(), sizeof(*this));                                         \
      memset(r.self(), 0, sizeof(*this));                                            \
   }

   template <typename Grow, typename Allocator>
   struct appender;

   // Inner slices are NOT bounds checked.
   // VPointers are NOT bounds checked.
   template <typename Allocator = A>
   struct trusted_view : public root<trusted_accessors, Allocator> {
      using Fixed = trusted_accessors;                       // Bounds have already been verified
      using Inner = trusted_slice<trusted_view<Allocator>>;  // Bounds have already been verified
      using Block = trusted_block<trusted_view<Allocator>>;

      inline wap::u8* voffset(wap::size_t offset) noexcept {
         if (WAP_UNLIKELY(offset < sizeof(SizeT))) return nullptr;
         return this->voffset_of(offset);
      }

      Fixed fixed(wap::size_t offset, wap::size_t size) { return Fixed(this->base_ + offset); }

      Inner inner(wap::size_t offset) { return Inner(*this, this->base() + offset); }

      inline std::string_view string(wap::size_t offset) {
         if (WAP_UNLIKELY(offset < BASE_SIZE_PLUS_SIZE)) return {};
         const auto data = this->voffset_of(offset);
         if constexpr (sizeof(SizeT) == 2) {
            return std::string_view((const char*)data + sizeof(SizeT), wap::bits::le::u16(data));
         } else if constexpr (sizeof(SizeT) == 4) {
            return std::string_view((const char*)data + sizeof(SizeT), wap::bits::le::u32(data));
         } else if constexpr (sizeof(SizeT) == 8) {
            return std::string_view((const char*)data + sizeof(SizeT), wap::bits::le::u64(data));
         }
         return {};
      }

      inline std::optional<std::string_view> optional_string(wap::size_t offset) {
         if (WAP_UNLIKELY(offset < BASE_SIZE_PLUS_SIZE)) return std::nullopt;
         const auto data = this->voffset_of(offset);
         if constexpr (sizeof(SizeT) == 2) {
            return std::string_view((const char*)data + sizeof(SizeT), wap::bits::le::u16(data));
         } else if constexpr (sizeof(SizeT) == 4) {
            return std::string_view((const char*)data + sizeof(SizeT), wap::bits::le::u32(data));
         } else if constexpr (sizeof(SizeT) == 8) {
            return std::string_view((const char*)data + sizeof(SizeT), wap::bits::le::u64(data));
         }
         return std::nullopt;
      }

      inline std::optional<Block> optional_block(wap::size_t offset) {
         if (WAP_UNLIKELY(offset < BASE_SIZE_PLUS_SIZE)) return std::nullopt;
         return {*this, this->voffset_of(offset)};
      }

      explicit trusted_view(wap::u8* data, wap::u8* base)
          : root<trusted_accessors, Allocator>(data, base) {}

      //      template <typename G>
      //      trusted_view(G&& a) : root<trusted_accessors>(a.data_, a.base_) {
      //         this->data_ = a.data_;
      //         a.data_ = nullptr;
      //      }
      //
      //      template <typename G>
      //      trusted_view& operator=(appender<G>&& a) {
      //        this->data_ = a.data_;
      //        a.data_ = nullptr;
      //      }

      WAP_VIEW_CTORS(trusted_view, trusted_accessors, r.data_, r.base())
   };

   // Inner slices are NOT bounds checked.
   // VPointers are bounds checked.
   template <typename Allocator = A>
   struct partial_trusted_view : public root<trusted_accessors, Allocator> {
      using Fixed = trusted_accessors;  // Bounds have already been verified
      using Inner = partial_trusted_slice<partial_trusted_view<Allocator>>;  // Bounds have already
                                                                             // been verified
      using Block = partial_trusted_block<partial_trusted_view<Allocator>>;

      inline wap::u8* voffset(wap::size_t offset) noexcept {
         const auto data = this->voffset_of(offset);
         if (WAP_UNLIKELY(data < this->base_ || data > end_ - sizeof(SizeT))) return nullptr;
         return data;
      }

      Fixed fixed(wap::size_t offset, wap::size_t size) const { return Fixed(this->voffset_of(offset)); }

      Inner inner(wap::size_t offset) const { return Inner(*this, this->voffset_of(offset)); }

      std::string_view string(wap::size_t offset) {
         const auto data = this->voffset_of(offset);
         if (WAP_UNLIKELY(data < this->base_ || data > end_ - sizeof(SizeT))) return {};
         if constexpr (sizeof(SizeT) == 2) {
            const auto size = wap::bits::le::u16(data);
            if (WAP_UNLIKELY(data + size > end_)) return {};
            return std::string_view((const char*)data + 2, size);
         } else if constexpr (sizeof(SizeT) == 4) {
            const auto size = wap::bits::le::u32(data);
            if (WAP_UNLIKELY(data + size > end_)) return {};
            return std::string_view((const char*)data + 4, size);
         }
         return {};
      }

      std::optional<std::string_view> optional_string(wap::size_t offset) {
         const auto data = this->voffset_of(offset);
         if (WAP_UNLIKELY(data < this->base_ || data > end_ - sizeof(SizeT))) return std::nullopt;
         if constexpr (sizeof(SizeT) == 2) {
            const auto size = wap::bits::le::u16(data);
            if (WAP_UNLIKELY(data + size > end_)) return std::nullopt;
            return std::string_view((const char*)data + 2, size);
         } else if constexpr (sizeof(SizeT) == 4) {
            const auto size = wap::bits::le::u32(data);
            if (WAP_UNLIKELY(data + size > end_)) return std::nullopt;
            return std::string_view((const char*)data + 4, size);
         }
         return std::nullopt;
      }

      std::optional<Block> optional_block(wap::size_t offset) {
         const auto data = this->voffset_of(offset);
         // bounds check offset
         if (WAP_UNLIKELY(data < this->base_ || data > end_ - sizeof(block)
                          || data + (wap::size_t)((msg::block*)data)->get_size() > end_))
            return std::nullopt;

         return {*this, (wap::u8*)data};
      }

      inline wap::u8* end() { return end_; }

      explicit partial_trusted_view(wap::u8* data, wap::u8* base, wap::u8* end)
          : root<trusted_accessors, Allocator>(data, base), end_(end) {}

      WAP_VIEW_CTORS(partial_trusted_view, trusted_accessors, r.data_, r.base())
    private:
      wap::u8* end_;
   };

   // Inner slices are bounds checked.
   // VPointers are bounds checked.
   template <typename Allocator = A>
   struct untrusted_view : public root<untrusted_accessors, Allocator> {
      using Fixed = untrusted_accessors;
      using Inner = untrusted_slice<untrusted_view<Allocator>>;
      using Block = untrusted_block<untrusted_view<Allocator>>;

      inline wap::u8* voffset(wap::size_t offset) noexcept {
         const auto data = this->voffset_of(offset);
         if (WAP_UNLIKELY(data < this->base_ || data > end_ - sizeof(SizeT))) return nullptr;
         return data;
      }

      std::string_view string(wap::size_t offset) {
         const auto data = this->voffset_of(offset);
         if (WAP_UNLIKELY(data < this->base_ || data > end_ - sizeof(SizeT))) return {};
         if constexpr (sizeof(SizeT) == 2) {
            const auto size = wap::bits::le::u16(data);
            if (WAP_UNLIKELY(data + size > end_)) return {};
            return std::string_view((const char*)data + 2, size);
         } else if constexpr (sizeof(SizeT) == 4) {
            const auto size = wap::bits::le::u32(data);
            if (WAP_UNLIKELY(data + size > end_)) return {};
            return std::string_view((const char*)data + 4, size);
         }
         return {};
      }

      std::optional<std::string_view> optional_string(wap::size_t offset) {
         const auto data = this->voffset_of(offset);
         if (WAP_UNLIKELY(data < this->base_ || data > end_ - sizeof(SizeT))) return std::nullopt;
         if constexpr (sizeof(SizeT) == 2) {
            const auto size = wap::bits::le::u16(data);
            if (WAP_UNLIKELY(data + size > end_)) return std::nullopt;
            return std::string_view((const char*)data + 2, size);
         } else if constexpr (sizeof(SizeT) == 4) {
            const auto size = wap::bits::le::u32(data);
            if (WAP_UNLIKELY(data + size > end_)) return std::nullopt;
            return std::string_view((const char*)data + 4, size);
         }
         return std::nullopt;
      }

      std::optional<Block> optional_block(wap::size_t offset) {
         const auto data = this->voffset_of(offset);
         // bounds check offset
         if (WAP_UNLIKELY(data < this->base_ || data > end_ - sizeof(block)
                          || data + (wap::size_t)((msg::block*)data)->get_size() > end_))
            return std::nullopt;

         return {*this, (wap::u8*)data};
      }

      inline wap::u8* end() { return end_; }

      explicit untrusted_view(wap::u8* data, wap::u8* base, wap::u8* base_end, wap::u8* end)
          : root<untrusted_accessors, Allocator>(data, base, base_end), end_(end) {}

      WAP_VIEW_CTORS(untrusted_view, untrusted_accessors, r.data_, r.base(), r.base_end())
    private:
      wap::u8* end_;
   };

#undef WAP_VIEW_CTORS

   template <typename Allocator = A>
   using ViewTrusted = typename Model::template view<trusted_view<Allocator>>;
   template <typename Allocator = A>
   using ViewPartial = typename Model::template view<partial_trusted_view<Allocator>>;
   template <typename Allocator = A>
   using ViewUntrusted = typename Model::template view<untrusted_view<Allocator>>;
   template <typename Allocator = A>
   using ViewVariant
      = std::variant<ViewTrusted<Allocator>, ViewPartial<Allocator>, ViewUntrusted<Allocator>>;

   template <typename Allocator = A>
   inline static wap::result<ViewVariant<Allocator>, wap::parse_error> parse(std::string&& s) {
      return parse<Allocator>((wap::u8*)s.data(), (wap::size_t)s.size());
   }
   template <typename Allocator = A>
   inline static wap::result<ViewVariant<Allocator>, wap::parse_error> parse(std::string_view s) {
      return parse<Allocator>((wap::u8*)s.data(), (wap::size_t)s.size());
   }
   // parse takes a raw buffer and size and creates a ViewParseVariant.
   // It performs basic bounds checks on total message size and ensures
   // base size is well-formed.
   template <typename Allocator = A>
   inline static wap::result<ViewVariant<Allocator>, wap::parse_error> parse(
      wap::u8* data, wap::size_t size) {
      if (WAP_UNLIKELY(!data || size < HEADER_SIZE))
         return wap::parse_error{wap::parse_error::reason_code::malformed};

      const size_t sz = Header::size(data);
      const size_t base_size = Header::base_size(data);
      const size_t header_size = Header::SIZE;

      // Is it malformed or perhaps the wrong message type?
      if (WAP_UNLIKELY(sz > size || ((wap::size_t)Header::SIZE + base_size > size)))
         return wap::parse_error{wap::parse_error::reason_code::overflow};

      if (base_size < BASE_SIZE) {
         return ViewVariant<Allocator>{std::in_place_type<ViewUntrusted<Allocator>>, data,
            data + Header::SIZE, data + HEADER_SIZE + base_size, data + sz};
      }
      return ViewVariant<Allocator>{
         std::in_place_type<ViewPartial<Allocator>>, data, data + Header::SIZE, data + sz};
   }

   //   template <typename... Ts>
   //   struct match : Ts... {
   //      using Ts::operator()...;
   //   };
   //   template <class... Ts>
   //   match(Ts...) -> match<Ts...>;

   //   template <class... Vs>
   //   inline static void visit(wap::u8* data, wap::size_t size, Vs... visitor) {
   //      auto parsed = wap::msg<M, SizeT, H, wap::allocator::leaking>::parse(data, size);
   //      std::visit(wap::match<Vs...>{std::forward<decltype(visitor)>(visitor)...}, parsed);
   //   }



   template <class... Vs>
   inline static std::optional<wap::parse_error> visit(
      wap::u8* data, wap::size_t size, Vs... visitor) noexcept {
      auto parsed = parse<wap::allocator::noop>(data, size);

      if (std::holds_alternative<wap::parse_error>(parsed)) {
         return std::get<wap::parse_error>(parsed);
      }

      std::visit(wap::match<Vs...>{std::forward<decltype(visitor)>(visitor)...},
         std::get<ViewVariant<wap::allocator::noop>>(parsed));
      return std::nullopt;
   }

   template <class... Vs>
   inline static std::optional<wap::parse_error> visit(std::string_view msg, Vs... visitor) {
      return visit((wap::u8*)msg.data(), (wap::size_t)msg.size(), visitor...);
   }

   template <class... Vs>
   inline static void visit_or_throw(wap::u8* data, wap::size_t size, Vs... visitor) {
      auto parsed = parse<wap::allocator::noop>(data, size);

      if (std::holds_alternative<wap::parse_error>(parsed)) {
         throw std::get<wap::parse_error>(parsed);
      }

      std::visit(wap::match<Vs...>{std::forward<decltype(visitor)>(visitor)...},
         std::get<ViewVariant<wap::allocator::noop>>(parsed));
   }

   inline static construct_result construct(size_t extra = BASE_SIZE / 2) {
      const auto message_size = MINIMAL_CONSTRUCT_SIZE;
      const auto buffer_size = MINIMAL_CONSTRUCT_SIZE + extra;
      auto data = (u8*)Allocator::allocate(buffer_size);

      // TODO: Remove!
      memset(data, 0, buffer_size);

      if (WAP_UNLIKELY(!data)) return {nullptr, nullptr, nullptr, nullptr};

      Header::set_message_size(data, message_size);
      Header::set_base_size(data, BASE_SIZE);
      auto base = Header::base(data);

      return {data, base, base + BASE_SIZE, data + buffer_size};
   }

   ////////////////////////////////////////////////////////////////////////////////////
   // BUILDERS
   ////////////////////////////////////////////////////////////////////////////////////

   // Fixed and Inner are NOT bounds checked.
   // VPointers are NOT bounds checked.
   template <typename R>
   struct block_builder_trusted : public trusted_accessors_builder<R> {
      using Fixed = trusted_accessors;
      using Inner = trusted_slice<R>;
      using Block = trusted_block<R>;

      inline block* as_block() { return (block*)this->base_; }

      inline SizeT size() { return as_block()->get_size(); }

      inline wap::u8* data() { return this->base_ + sizeof(SizeT); }

      std::string_view get() {
         return std::string_view((const char*)this->base_ + block::OVERHEAD, size());
      }

      Inner inner(wap::size_t offset, wap::size_t size) {
         return Inner(root(), this->base_ + offset);
      }

      Fixed fixed(wap::size_t offset, wap::size_t size) { return Fixed(this->base_ + offset); }

      inline std::string_view string(wap::size_t offset) { return root().string(offset); }

      inline std::optional<std::string_view> optional_string(wap::size_t offset) {
         return root().optional_string(offset);
      }

      inline std::optional<Block> optional_block(wap::size_t offset) {
         return root().optional_block(offset);
      }

      block_builder_trusted(R& root, wap::size_t offset)
          : trusted_accessors_builder<R>(root, offset) {}
   };

   template <typename B>
   struct block_builder {
      bool deallocate() {
         if (WAP_UNLIKELY(!data_)) return true;
         b_.deallocate(*vptr_);
         block_ = nullptr;
         data_ = nullptr;
         *vptr_ = 0;
         return true;
      }

      wap::u8* allocate(wap::size_t new_capacity) {
         if (!data_) {
            auto result = b_.allocate(new_capacity);
            if (WAP_UNLIKELY(!result.ptr)) {
               return nullptr;
            }
            block_ = (msg::block*)result.ptr;
            data_ = block_ + msg::block::OVERHEAD;
            *vptr_ = result.offset;
            return data_;
         }
         if (new_capacity < (wap::size_t)block_->get_size()) {
            return data_;
         }
         auto result = b_.reallocate(*vptr_, new_capacity);
         if (WAP_UNLIKELY(!result.ptr)) {
            return nullptr;
         }
         block_ = (msg::block*)result.ptr;
         data_ = block_ + msg::block::OVERHEAD;
         *vptr_ = result.offset;
         return data_;
      }

      wap::u8* extend(wap::size_t by) {
         if (WAP_UNLIKELY(!block_)) {
            return allocate(by);
         }
         const auto new_capacity = (wap::size_t)block_->get_size() + by;
         auto result = b_.reallocate(*vptr_, new_capacity);
         if (WAP_UNLIKELY(!result.ptr)) {
            return nullptr;
         }
         block_ = (msg::block*)result.ptr;
         data_ = block_ + msg::block::OVERHEAD;
         *vptr_ = result.offset;
         return data_;
      }

      wap::u8* shrink(wap::size_t new_capacity) {
         if (WAP_UNLIKELY(!block_)) {
            auto result = b_.allocate(new_capacity);
            if (WAP_UNLIKELY(!result.ptr)) {
               return nullptr;
            }
            block_ = (msg::block*)result.ptr;
            data_ = block_ + msg::block::OVERHEAD;
            *vptr_ = result.offset;
            return data_;
         }
         const auto existing_capacity = (wap::size_t)block_->get_size();
         if (WAP_UNLIKELY(new_capacity == existing_capacity)) return data_;

         auto result = b_.reallocate(*vptr_, new_capacity);
         if (WAP_UNLIKELY(!result.ptr)) {
            return nullptr;
         }
         block_ = (msg::block*)result.ptr;
         data_ = block_ + msg::block::OVERHEAD;
         *vptr_ = result.offset;
         return data_;
      }

      bool set(wap::u8* data, wap::size_t size) {
         if (WAP_UNLIKELY(!data || !size)) return false;
         auto d = allocate(size);
         if (WAP_UNLIKELY(!d)) return false;
         ::memcpy(d, data, size);
         return true;
      }

    protected:
      B& b_;
      wap::vpointer_t<SizeT>* vptr_;
      msg::block* block_;
      wap::u8* data_;
      wap::u8* end_;
   };

   template <typename Grow = wap::grow::doubled, typename Allocator = A>
   struct appender : public root<trusted_accessors_mutable> {
      using Fixed = trusted_accessors_builder<appender<Grow>>;  // Bounds have already been verified
      using Inner = trusted_slice<trusted_view<Allocator>>;     // Bounds have already been verified
      using Block = trusted_block<trusted_view<Allocator>>;

      using Finish = ViewTrusted<Allocator>;
      using FinishView = trusted_view<Allocator>;

      bool grow(wap::size_t by) {
         const size_t new_capacity = Grow::calc(capacity(), by);
         const size_t existing_size = tail_ - this->data_;

         // can't shrink
         if (WAP_UNLIKELY(new_capacity < (size_t)capacity())) return false;

         // try to reallocate
         const auto new_buffer = Allocator::reallocate(this->data_, new_capacity);
         if (WAP_UNLIKELY(!new_buffer)) return false;

         // in place reallocation?
         if (WAP_UNLIKELY(this->data_ == new_buffer)) {
            // ensure the end is correctly set
            end_ = new_buffer + new_capacity;
            return true;
         }

         this->data_ = new_buffer;
         this->base_ = new_buffer + Header::SIZE;
         tail_ = new_buffer + existing_size;
         end_ = new_buffer + new_capacity;
         return true;
      }

      alloc_result append(wap::size_t size) {
         auto const current_size = this->size();
         auto const new_size = current_size + block::OVERHEAD + size;
         auto new_tail = tail_ + block::OVERHEAD + size;

         // Grow?
         if (WAP_UNLIKELY(new_tail > end_)) {
            if (WAP_UNLIKELY(!grow(size + block::OVERHEAD))) return {nullptr, 0, 0};

            new_tail = tail_ + block::OVERHEAD + size;
         }

         const auto blk = tail_;
         tail_ = new_tail;

         // Set the new message size.
         Header::set_size(this->data_, new_size);
         // Set the block size.
         Header::set_size(blk, size);

         return {blk, blk - this->base_block(), size};
      }

      alloc_result allocate(wap::size_t size) { return append(size); }

      void deallocate(wap::size_t offset) {
         const wap::size_t current_size = this->size();

         // Out of bounds?
         if (WAP_UNLIKELY(offset < BASE_SIZE_PLUS_SIZE || offset > current_size - sizeof(block)))
            return;

         // Get block.
         auto blk = (msg::block*)this->voffset_of(offset);

         // Dangling pointer?
         if (WAP_UNLIKELY(blk->is_free())) return;

         const auto block_size = blk->get_size();

         // Is it the last allocated block?
         if (WAP_LIKELY(current_size == offset + msg::block::OVERHEAD + block_size)) {
            Header::set_size(this->data_, offset);
            return;
         }

         trash_ += block_size + msg::block::OVERHEAD;
      }

      void free(wap::u8* ptr) {
         const auto base = this->base_block();
         if (WAP_UNLIKELY(ptr < base + BASE_SIZE_PLUS_SIZE)) return;
         deallocate(ptr - base);
      }

      alloc_result reallocate(wap::size_t offset, wap::size_t size) {
         const wap::size_t current_size = this->size();

         // Out of bounds?
         if (WAP_UNLIKELY(offset < BASE_SIZE_PLUS_SIZE || offset > current_size - sizeof(block)))
            return {nullptr, 0, 0};

         // Get block.
         auto blk = (msg::block*)this->voffset_of(offset);

         // Dangling pointer?
         if (WAP_UNLIKELY(blk->is_free())) return {nullptr, 0, 0};

         // Get current buffer size.
         const auto current_block_size = blk->get_size();

         // Is it the last allocation?
         if (current_size == offset + msg::block::OVERHEAD + current_block_size) {
            auto new_size = current_size - current_block_size + size;
            Header::set_size(this->data_, new_size);
            Header::set_size(this->base_block() + offset, size);
            return {(wap::u8*)blk, offset, size};
         }

         // Can it fit in current size?
         if (current_block_size >= size) {
            trash_ += current_block_size - size;
            return {(wap::u8*)blk, offset, current_block_size};
         }

         // Create a new allocation and copy over.
         const auto new_allocation = append(size);
         if (WAP_UNLIKELY(!new_allocation.ptr)) return {nullptr, 0, 0};
         ::memcpy(new_allocation.ptr + msg::block::OVERHEAD, (void*)(blk + msg::block::OVERHEAD),
            WAP_MIN(size, current_block_size));

         // Free the previous block and increment the trash.
         blk->set_free();
         trash_ += current_block_size + msg::block::OVERHEAD;

         // Return new allocation.
         return new_allocation;
      }

      inline wap::size_t capacity() { return end_ - this->data_; }

      inline std::string_view string(wap::size_t offset) {
         if (WAP_UNLIKELY(offset < BASE_SIZE_PLUS_SIZE)) return {};
         const auto data = this->voffset_of(offset);
         if constexpr (sizeof(SizeT) == 2) {
            const auto size = wap::bits::le::u16(data);
            return std::string_view((const char*)data + 2, size);
         } else if constexpr (sizeof(SizeT) == 4) {
            const auto size = wap::bits::le::u32(data);
            return std::string_view((const char*)data + 4, size);
         }
         return {};
      }

      inline std::optional<std::string_view> optional_string(wap::size_t offset) {
         if (WAP_UNLIKELY(offset < BASE_SIZE_PLUS_SIZE)) return std::nullopt;
         const auto data = this->voffset_of(offset);
         // handle nullptr
         if constexpr (sizeof(SizeT) == 2) {
            const auto size = wap::bits::le::u16(data);
            return std::string_view((const char*)data + 2, size);
         } else if constexpr (sizeof(SizeT) == 4) {
            const auto size = wap::bits::le::u32(data);
            return std::string_view((const char*)data + 4, size);
         }
         return std::nullopt;
      }

      inline std::optional<Block> optional_block(wap::size_t offset) {
         if (WAP_UNLIKELY(offset < BASE_SIZE_PLUS_SIZE)) return std::nullopt;
         return {*this, this->voffset_of(offset)};
      }

      ViewTrusted<Allocator> finish() { return ViewTrusted(std::move(this)); }

      appender(wap::u8* data, wap::u8* base, wap::u8* tail, wap::u8* end)
          : root<trusted_accessors_mutable>(data, &this->data_, (wap::size_t)Header::SIZE),
            tail_(tail),
            end_(end),
            trash_(0) {}

      using Builder = typename Model::template builder<appender>;
      static Builder make(wap::size_t extra = 64) {
         auto [data, base, tail, end] = msg::construct(extra);
         return Builder{data, base, tail, end};
      }

      friend class trusted_view<Allocator>;

    private:
      wap::u8* tail_;
      wap::u8* end_;
      wap::size_t trash_;
   };

   template <typename Grow = wap::grow::doubled>
   class arena_embedded {};

   template <typename S, typename Grow = wap::grow::doubled,
      typename FreeList = wap::small::free_list>
   class arena {};

   template <typename Grow = wap::grow::doubled>
   using Appender = typename Model::template builder<appender<Grow>>;
};

//  template <typename Builder, typename View = typename Builder::Finish> View freeze(Builder&& b) {
//    return b.finish();
//  }

// Memory is allocated in contiguous blocks starting with the base block.
//
// There are several implementation subtleties involved:
// - The prev_phys field is only valid if the previous block is free.
// - The prev_phys field is actually stored at the end of the
//   previous block. It appears at the beginning of this structure only to
//   simplify the implementation.
// - The next_free / prev_free fields are only valid if the block is free.
struct block_header {
   using SizeT = voffset_t;

   SizeT prev_phys;
   SizeT size;
   SizeT next_free;
   SizeT prev_free;

   // User data starts directly after the size field in a used block.

   // Since block sizes are always at least a multiple of 4, the two least
   // significant bits of the size field are used to store the block status:
   // - bit 0: whether block is used or free
   // - bit 1: whether previous block is used or free
   constexpr static const SizeT FREE_BIT = 1 << 0;
   constexpr static const SizeT PREV_FREE_BIT = 1 << 1;

   // The size of the block header exposed to used blocks is the size field.
   // The prev_phys field is stored *inside* the previous free block.
   constexpr static const SizeT OVERHEAD = sizeof(SizeT);
   constexpr static const SizeT DATA_OFFSET = sizeof(SizeT) * 2;

   /* User data starts directly after the size field in a used block. */

   WAP_NODISCARD inline SizeT get_size() const { return size & ~(FREE_BIT | PREV_FREE_BIT); }

   inline void set_size(SizeT value) { size = value | (size & (FREE_BIT | PREV_FREE_BIT)); }

   WAP_NODISCARD inline bool is_free() const { return size & FREE_BIT; }

   inline void set_free() { size |= FREE_BIT; }

   inline void set_used() noexcept { size &= ~FREE_BIT; }

   WAP_NODISCARD inline bool is_prev_free() const noexcept { return size & PREV_FREE_BIT; }

   inline void set_prev_free() noexcept { size |= PREV_FREE_BIT; }

   inline void set_prev_used() noexcept { size &= ~PREV_FREE_BIT; }

   constexpr static inline block_header* from_ptr(const u8* ptr) {
      return WAP_CAST(block_header*, ptr - DATA_OFFSET);
   }

   WAP_NODISCARD inline u8* to_ptr() const noexcept { return WAP_CAST(u8*, this) + DATA_OFFSET; }
};

static_assert(sizeof(block_header) == sizeof(uint32_t) * 4);

class walker {
 public:
   using callback_fn = void(
      u8* ptr, uint32_t block_offset, uint32_t ptr_offset, uint32_t size, bool used, void* user);

   constexpr static void print_walker(uint8_t* ptr, uint32_t block_offset, uint32_t ptr_offset,
      uint32_t size, bool used, void* user) {
      (void)user;
      printf("[%s]  offset: %u  size: %u \t(%p)\n", used ? "used" : "free", (uint32_t)ptr_offset,
         (uint32_t)size, (void*)block_header::from_ptr(ptr));
   }

   static void print(uint8_t* buffer, void* user = nullptr) {
      walk(print_walker, buffer, user);
      printf("\n");
   }

   static void walk(callback_fn walker, uint8_t* buffer, void* user) {
      auto total_size = ::wap::bits::le::u32(buffer);

      auto block = (block_header*)buffer;
      auto offset = (uint32_t)WAP_POINTER_DIFF(block, buffer);

      while (block) {
         auto const size = block->get_size();
         auto const is_free = block->is_free();

         walker(block->to_ptr(), offset, offset + block_header::DATA_OFFSET, size, !is_free, user);

         offset += size + block_header::OVERHEAD;

         if (offset >= total_size) {
            break;
         }

         block = (block_header*)WAP_POINTER_OFFSET(buffer, offset);
      }
   }
};

template <typename Allocator = ::wap::allocator>
class view {
 public:
   using SizeT = block_header::SizeT;

   view() = delete;
   view(u8* buffer) : buffer_(buffer) {}

   // no copy
   view(view& v) = delete;
   view& operator=(view& r) = delete;

   // ok move
   view(view&& v) noexcept = default;
   view& operator=(view&& r) noexcept = default;

   WAP_NODISCARD inline SizeT size() const noexcept { return SizeT(bits::le::u32(buffer_)); }

   // root struct size without any dynamic allocations
   WAP_NODISCARD inline SizeT base_size() const noexcept {
      return (SizeT)bits::le::u32(buffer_ + sizeof(uint32_t));
   }

   ~view() {
      if (WAP_LIKELY(buffer_)) {
         Allocator::deallocate(buffer_);
         buffer_ = nullptr;
      }
   }

   class slice {
      static const uint32_t NULL_BLOCK = 0;
      constexpr static const ::wap::u8* NULL_SLICE = (::wap::u8*)&NULL_BLOCK;

    public:
      inline ::wap::u8* data() const noexcept { return slice_; }
      inline ::wap::u32 size() const noexcept { return size_; }
      inline ::wap::u8* const message_data() const noexcept { return view_.buffer_; }
      inline ::wap::u32 message_size() const noexcept { return view_.size(); }
      inline ::wap::u32 message_base_size() const noexcept { return view_.base_size(); }

      slice() = delete;
      slice(slice& s) = delete;
      slice(const slice& s) = delete;
      slice& operator=(const slice& s) = delete;
      slice& operator=(slice& s) = delete;

      // move ok
      slice(slice&& s) noexcept = default;
      slice(const slice&& s) noexcept = default;
      slice& operator=(slice&& s) noexcept = default;

      slice(wap::buffer<Allocator>& buffer, ::wap::u8* slice, ::wap::u32 size)
          : view_(buffer), slice_(slice), size_(size) {}

      slice inner(::wap::u32 offset, ::wap::u32 bounds) {
         if (WAP_UNLIKELY(offset >= size_)) return {view_, NULL_SLICE, 0};
         return {view_, (::wap::u8*)(slice_ + offset), WAP_MIN(bounds, size_) - offset};
      }

      ::wap::view<Allocator>::slice block(wap::u32 offset) {
         // bounds check offset
         if (WAP_UNLIKELY(offset < block_header::OVERHEAD || offset >= view_.size()))
            return {view_, NULL_SLICE, 0};

         // deref block
         auto block
            = (block_header*)WAP_POINTER_OFFSET(view_.buffer_, offset - block_header::DATA_OFFSET);

         // bounds check block
         const auto block_size = block->get_size();
         if (WAP_UNLIKELY(offset + block_size > view_.size())) return {view_, NULL_SLICE, 0};

         return {view_, (wap::u8*)(view_.buffer_ + offset), block_size};
      }

    public:
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

    private:
      ::wap::view<Allocator>& view_;
      ::wap::u8* slice_;
      ::wap::u32 size_;
   };

   static view<Allocator> make(SizeT base_size, SizeT capacity) {
      auto buffer = (u8*)Allocator::allocate((size_t)capacity);
      memset(buffer, 0, capacity);

      ptrdiff_t base_offset = 0;
      auto& base_block = *(block_header*)WAP_POINTER_OFFSET(buffer, (ptrdiff_t)base_offset);

      // put base_block capacity
      wap::bits::le::put_u32(buffer + base_offset, base_size);
   }

 protected:
   u8* buffer_;
};

class builder {
 public:
   template <typename Buffer>
   class slice {
      static const uint32_t NULL_BLOCK_HEADER = 0;
      constexpr static ::wap::u8* NULL_BLOCK = (::wap::u8*)&NULL_BLOCK_HEADER;
      constexpr static ::wap::voffset_t* NULL_VPOINTER = (::wap::voffset_t*)&NULL_BLOCK_HEADER;
      constexpr static ::wap::u8* NULL_SLICE = (::wap::u8*)&NULL_BLOCK_HEADER;

    public:
      inline ::wap::u8* data() const noexcept { return slice_; }
      inline ::wap::u32 size() const noexcept { return size_; }
      inline ::wap::u8* const message_data() const noexcept { return b_.buffer_; }
      inline ::wap::u32 message_size() const noexcept { return b_.size(); }
      inline ::wap::u32 message_base_size() const noexcept { return b_.base_size(); }

      slice() = delete;
      slice(slice& s) = delete;
      slice(const slice& s) = delete;
      slice& operator=(const slice& s) = delete;
      slice& operator=(slice& s) = delete;

      // move ok
      slice(slice&& s) noexcept = default;
      slice(const slice&& s) noexcept = default;
      slice& operator=(slice&& s) noexcept = default;
      slice& operator=(const slice&& s) noexcept = default;

      slice(Buffer& b, ::wap::u8* slice, ::wap::u32 size) : b_(b), slice_(slice), size_(size) {}

      // Blocks represent a dynamic allocation owned by a virtual pointer address.
      // block_header represents the physical layout which essentially dictates that
      // each block has a 4byte size field. A block may be null as evident by the
      // vpointer containing the 0 value.
      class block {
         static const uint32_t DATA = 0;

       public:
         inline bool is_valid() { return (uint32_t*)vptr_ != &DATA; }
         inline bool is_null() { return *(uint32_t*)data_; }

         inline u32 capacity() {
            return ::wap::bits::le::u32(data_ - block_header::OVERHEAD)
                   & ~(block_header::FREE_BIT | block_header::PREV_FREE_BIT);
         }

         inline u32 capacity_unsafe() {
            return ::wap::bits::le::u32(data_ - block_header::OVERHEAD)
                   & ~(block_header::FREE_BIT | block_header::PREV_FREE_BIT);
         }

         inline u8* data() { return data_; }

         inline voffset_t* vptr() { return vptr_; }

         block() = delete;
         block(block& s) = delete;
         block(const block& s) = delete;
         block& operator=(const block& s) = delete;
         block& operator=(block& s) = delete;

         // move ok
         block(block&& s) noexcept = default;
         //      block(const block&& s) noexcept = default;
         block& operator=(block&& s) noexcept = default;

         block(Buffer& buffer) : buffer_(buffer), vptr_((voffset_t*)DATA), data_((wap::u8*)DATA) {}

         block(Buffer& buffer, voffset_t* vptr, u8* data)
             : buffer_(buffer), vptr_(vptr), data_(data) {}

         bool deallocate() {
            if (WAP_UNLIKELY(is_null())) return true;
            buffer_.deallocate(*vptr_);
            data_ = nullptr;
            *vptr_ = 0;
            return true;
         }

         u8* allocate(u32 new_capacity) {
            if (WAP_UNLIKELY(!is_valid())) return nullptr;
            if (is_null()) {
               auto result = buffer_.allocate(new_capacity);
               if (WAP_UNLIKELY(!result.ptr)) {
                  return nullptr;
               }
               data_ = result.ptr;
               *vptr_ = result.offset;
               return data_;
            }
            if (new_capacity < capacity_unsafe()) {
               return data_;
            }
            auto result = buffer_.reallocate(*vptr_, new_capacity);
            if (WAP_UNLIKELY(!result.ptr)) {
               return nullptr;
            }
            data_ = result.ptr;
            *vptr_ = result.offset;
            return data_;
         }

         u8* extend(u32 by) {
            if (WAP_UNLIKELY(!is_valid())) return nullptr;
            if (WAP_UNLIKELY(is_null())) {
               return allocate(by);
            }
            const auto new_capacity = capacity_unsafe() + by;
            auto result = buffer_.reallocate(*vptr_, new_capacity);
            if (WAP_UNLIKELY(!result.ptr)) {
               return nullptr;
            }
            data_ = result.ptr;
            *vptr_ = result.offset;
            return data_;
         }

         u8* shrink(u32 new_capacity) {
            if (WAP_UNLIKELY(!is_valid())) return nullptr;
            if (is_null()) {
               auto result = buffer_.allocate(new_capacity);
               if (WAP_UNLIKELY(!result.ptr)) {
                  return nullptr;
               }
               data_ = result.ptr;
               *vptr_ = result.offset;
               return data_;
            }
            const auto existing_capacity = capacity_unsafe();
            if (WAP_UNLIKELY(new_capacity == existing_capacity)) return data_;

            auto result = buffer_.reallocate(*vptr_, new_capacity);
            if (WAP_UNLIKELY(!result.ptr)) {
               return nullptr;
            }
            data_ = result.ptr;
            *vptr_ = result.offset;
            return data_;
         }

         bool set(u8* data, u32 size) {
            if (WAP_UNLIKELY(!is_valid())) return false;
            auto d = allocate(size);
            if (WAP_UNLIKELY(!d)) return false;
            ::memcpy(d, data, size);
            return true;
         }

       protected:
         Buffer& buffer_;
         voffset_t* vptr_;
         u8* data_;
      };

      slice inner(::wap::u32 offset, ::wap::u32 bounds) {
         if (WAP_UNLIKELY(offset >= size_)) return {b_, NULL_SLICE, 0};
         return {b_, (::wap::u8*)(slice_ + offset), WAP_MIN(bounds, size_) - offset};
      }

      wap::builder::slice<Buffer>::block block(::wap::u32 voffset, ::wap::u32 vbounds) {
         if (WAP_UNLIKELY(vbounds > size())) return {b_};
         auto vptr = (voffset_t*)WAP_POINTER_OFFSET(slice_, voffset);
         const auto offset = *vptr;

         // bounds check offset
         if (WAP_UNLIKELY(offset < block_header::OVERHEAD || offset >= b_.size())) {
            return {b_};
         }

         auto block_size = ::wap::bits::le::u32(b_.data() + offset - block_header::OVERHEAD)
                           & ~(block_header::FREE_BIT | block_header::PREV_FREE_BIT);

         if (WAP_UNLIKELY(offset + block_size > b_.size())) return {b_};

         return {b_, vptr, b_.data() + offset};
      }

      //      std::optional<wap::builder::block<Buffer>> maybe_block(::wap::u32 voffset,
      //                                                             ::wap::u32 vbounds) {
      //        if (WAP_UNLIKELY(vbounds > size())) return {b_, NULL_VPOINTER, NULL_BLOCK_HEADER};
      //        const voffset_t* vptr = (voffset_t*)WAP_POINTER_OFFSET(s_, voffset);
      //        const auto offset = *vptr;
      //
      //        // bounds check offset
      //        if (WAP_UNLIKELY(offset < block_header::OVERHEAD || offset >= b_.size()))
      //          return {b_, NULL_VPOINTER, NULL_BLOCK_HEADER};
      //
      //        // deref blk
      //        auto blk
      //            = (block_header*)WAP_POINTER_OFFSET(b_.buffer_, offset -
      //            block_header::DATA_OFFSET);
      //
      //        // bounds check blk
      //        const auto block_size = blk->get_size();
      //        if (WAP_UNLIKELY(offset + block_size > b_.size())) return {b_, NULL_VPOINTER,
      //        NULL_BLOCK_HEADER};
      //
      //        return {b_, vptr, blk};
      //      }

    public:
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

    private:
      Buffer& b_;
      ::wap::u8* slice_;
      ::wap::u32 size_;
   };

   template <typename Buffer>
   class str {
    public:
      str(typename wap::builder::slice<Buffer>::block block) : block_(block) {}

      str& operator=(::std::string_view value) {
         const auto d = (u8*)value.data();
         const auto s = (u32)value.size();
         block_.set(d, s);
         return *this;
      }

      bool operator==(::std::string_view value) {
         if (!this->data_) return !value.data();
         if (!value.data()) return false;
         if (value.size() != (size_t)block_.capacity()) return false;
         return ::memcmp(value.data(), block_.data(), value.size()) == 0;
      }

    protected:
      typename wap::builder::slice<Buffer>::block block_;
   };
};

// Append only builder.
template <typename Allocator = ::wap::allocator, typename GrowPolicy = ::wap::grow::doubled>
class appender {
 public:
   using SizeT = block_header::SizeT;

   appender(u8* buffer, SizeT capacity) noexcept
       : buffer_(buffer), capacity_(capacity), trash_(0) {}

   appender() = delete;

   // no copy
   appender(appender& b) = delete;
   appender& operator=(appender& r) = delete;

   // ok move
   appender(appender&& b) noexcept = default;
   appender& operator=(appender&& r) noexcept = default;

   ~appender() {
      if (WAP_LIKELY(buffer_)) {
         Allocator::deallocate(buffer_);
         buffer_ = nullptr;
      }
   }

   WAP_NODISCARD inline SizeT size() const noexcept { return SizeT(bits::le::u32(buffer_)); }

   // root struct size without any dynamic allocations
   WAP_NODISCARD inline SizeT base_size() const noexcept {
      return (SizeT)bits::le::u32(buffer_ + sizeof(uint32_t));
   }

   WAP_NODISCARD inline SizeT capacity() const noexcept { return capacity_; }

   WAP_NODISCARD inline u8* data() const noexcept { return buffer_; }

   void print(void* user = nullptr) { walker::print(buffer_, user); }

   wap::builder::slice<appender<Allocator, GrowPolicy>> base() {
      return wap::builder::slice<appender<Allocator, GrowPolicy>>(
         *this, buffer_ + sizeof(uint32_t) + block_header::OVERHEAD, base_size());
   }

   bool grow(SizeT by) {
      const size_t new_capacity = GrowPolicy::calc(capacity_, by);
      if (WAP_UNLIKELY(new_capacity < (size_t)capacity_)) return false;
      const auto new_buffer = Allocator::reallocate(buffer_, new_capacity);
      if (WAP_UNLIKELY(!new_buffer)) return false;
      buffer_ = new_buffer;
      capacity_ = (SizeT)new_capacity;
      return true;
   }

   alloc_result append(SizeT size) {
      auto const current_size = this->size();
      auto const new_size = current_size + block_header::OVERHEAD + size;

      // Grow?
      if (WAP_UNLIKELY(new_size > capacity_ && !grow(size + block_header::OVERHEAD))) {
         return {nullptr, 0, 0};
      }

      // Set the new size.
      ::wap::bits::le::put_u32(buffer_, new_size);
      // Set the block size.
      ::wap::bits::le::put_u32(buffer_ + current_size, size);

      const auto data_offset = current_size + block_header::OVERHEAD;

      return {(u8*)WAP_POINTER_OFFSET(buffer_, data_offset), data_offset, size};
   }

   alloc_result allocate(SizeT size) { return append(size); }

   alloc_result reallocate(voffset_t offset, SizeT size) {
      const SizeT block_offset = offset - block_header::DATA_OFFSET;

      // Out of bounds?
      if (WAP_UNLIKELY(block_offset > capacity_ - block_header::DATA_OFFSET))
         return {nullptr, 0, 0};

      // Get block.
      auto block = (block_header*)WAP_POINTER_OFFSET(buffer_, block_offset);

      // Dangling pointer?
      if (WAP_UNLIKELY(block->is_free())) return {nullptr, 0, 0};

      // Get current buffer size.
      const auto current_size = this->size();
      const auto current_block_size = block->get_size();

      // Is it the last allocation?
      if (current_size == offset + size) {
         auto new_size = current_size - current_block_size + size;
         // Set the block size to the new size.
         ::wap::bits::le::put_u32(buffer_ + offset - block_header::OVERHEAD, size);
         // Update the total size of the message.
         ::wap::bits::le::put_u32(buffer_, new_size);
         return {block->to_ptr(), offset, size};
      }

      // Can it fit in current size?
      if (current_block_size >= size) {
         trash_ += current_block_size - size;
         return {block->to_ptr(), offset, current_block_size};
      }

      // Create a new allocation and copy over.
      const auto new_allocation = append(size);
      if (WAP_UNLIKELY(!new_allocation.ptr)) return {nullptr, 0, 0};
      ::memcpy(new_allocation.ptr, block->to_ptr(), WAP_MIN(size, current_block_size));

      // Free the previous block and increment the trash.
      block->set_free();
      trash_ += current_block_size + block_header::OVERHEAD;

      // Return new allocation.
      return new_allocation;
   }

   void deallocate(voffset_t offset) {
      const SizeT current_size = this->size();
      const SizeT block_offset = offset - block_header::DATA_OFFSET;

      // Out of bounds?
      if (WAP_UNLIKELY(offset > current_size)) return;

      // Get block.
      auto block = (block_header*)WAP_POINTER_OFFSET(buffer_, block_offset);

      // Dangling pointer?
      if (WAP_UNLIKELY(block->is_free())) return;

      const auto block_size = block->get_size();

      // Is it the last allocated block?
      if (current_size == offset + block_size) {
         ::wap::bits::le::put_u32(buffer_, offset - block_header::OVERHEAD);
         return;
      }

      trash_ += block_size + block_header::OVERHEAD;
   }

   // convenience method that takes a raw pointer and converts to voffset_t
   void free(const u8* ptr) {
      const auto offset = WAP_POINTER_DIFF(ptr, buffer_);
      if (WAP_UNLIKELY(offset < 1 || offset > this->tail_)) return;
      deallocate((voffset_t)offset);
   }

   static appender make(SizeT base_size, SizeT capacity) {
      auto buffer = (u8*)Allocator::allocate((size_t)capacity);
      memset(buffer, 0, capacity);

      ptrdiff_t base_offset = 0;  //-4;  // sizeof(uint32_t);
      auto& base_block = *(block_header*)WAP_POINTER_OFFSET(buffer, (ptrdiff_t)base_offset);

      // put base_block capacity
      base_block.set_size(base_size);
      base_block.set_used();

      const ptrdiff_t last_offset = base_offset + base_block.get_size() + block_header::OVERHEAD;
      const auto last = (block_header*)WAP_POINTER_OFFSET(buffer, last_offset);
      //      auto last_offset_check = WAP_POINTER_DIFF(last, buffer);
      //      (void)last_offset_check;
      SizeT pool_bytes = (ptrdiff_t)capacity - last_offset - (ptrdiff_t)block_header::OVERHEAD;

      last->set_size(pool_bytes);
      last->set_free();
      last->next_free = 0;
      last->prev_free = 0;

      ::wap::bits::le::put_u32(buffer, last_offset + block_header::OVERHEAD);

      return appender(buffer, capacity);
   }

 protected:
   u8* buffer_;
   u8* end_;
   SizeT capacity_;
   SizeT trash_;
};

// Arena provides linear memory management for a message root. It uses a segmented
// free list with a bitmap to search for a free block of an equal or greater size class.
// The size of the freelist is determined by a size class layout.
//
// O(1) memory allocator that can be used in real-time systems. Uses the same block layout
// and linked-list structure as TLSF. However, it differs in the segmented list being
// a single level rather than the larger memory usage of the two level list. For messages
// that are less than 64kb, the freelist overhead is only 64bytes and can be optionally
// embedded in the buffer.
//
// Use this for highly mutable messages that benefit from a full memory manager.
// Always compare against the appender and rebuilding once the trash gets too high.
template <typename Allocator = ::wap::allocator, typename GrowthPolicy = ::wap::grow::doubled,
   typename FreeList = ::wap::small::free_list>
class arena {
 public:
   using BlockSizeT = block_header::SizeT;
   using SizeClass = typename FreeList::SizeClassT;
   using ListSizeT = typename FreeList::ListSizeT;
   using u8 = uint8_t;

   arena() = delete;

   // no copy
   arena(arena& b) = delete;
   arena& operator=(arena& r) noexcept = delete;

   // ok move
   arena(arena&& b) = default;
   arena& operator=(arena&& r) noexcept = default;

   ~arena() {
      if (WAP_LIKELY(buffer_)) {
         Allocator::deallocate(buffer_);
         buffer_ = nullptr;
      }
   }

   WAP_NODISCARD inline BlockSizeT size() const noexcept {
      return BlockSizeT(bits::le::u32(buffer_));
   }

   // root struct size without any dynamic allocations
   WAP_NODISCARD inline BlockSizeT base_size() const noexcept {
      return (BlockSizeT)bits::le::u32(buffer_ + sizeof(uint32_t));
   }

   WAP_NODISCARD inline BlockSizeT capacity() const noexcept { return capacity_; }

   WAP_NODISCARD inline u8* data() const noexcept { return buffer_; }

 private:
   constexpr static const u32 ALIGN = sizeof(BlockSizeT);

   constexpr static ListSizeT align_up(ListSizeT x, ListSizeT align) {
      WAP_ASSERT(0 == (align & (align - 1)) && "must align to a power of two");
      return (x + (align - 1)) & ~(align - 1);
   }

   constexpr static ListSizeT align_down(ListSizeT x, ListSizeT align) {
      WAP_ASSERT(0 == (align & (align - 1)) && "must align to a power of two");
      return x - (x & (align - 1));
   }

   inline bool is_block_inbounds(BlockSizeT offset) { return offset > 0 && offset <= tail_; }

   inline block_header* deref_unsafe(BlockSizeT offset) {
      return (block_header*)WAP_POINTER_OFFSET(buffer_, (ptrdiff_t)offset);
   }

#if WAP_UNSAFE_HEAP
   // dereference a block_header pointer from logical offset.
   inline block_header* deref_safe(SizeT offset) {
      return (block_header*)WAP_POINTER_OFFSET(buffer_, (ptrdiff_t)offset);
   }
#else
   // dereference a block_header pointer from logical offset.
   inline block_header* deref_safe(BlockSizeT offset) {
      if (WAP_UNLIKELY(!is_block_inbounds(offset))) return nullptr;
      return (block_header*)((ptrdiff_t)buffer_ + (ptrdiff_t)offset);
   }
#endif

   // Remove a given block from the free list.
   inline void remove(block_header* block, BlockSizeT offset) {
      const SizeClass cls = FreeList::pool_size((ListSizeT)block->get_size());
      auto prev = deref_safe(block->prev_free);
      auto next = deref_safe(block->next_free);
      if (next) {
         if (prev) {
            next->prev_free = block->prev_free;
            prev->next_free = block->next_free;
         } else {
            next->prev_free = 0;
         }
      } else if (prev) {
         prev->next_free = 0;
      }

      // If this block is the head of the free list, set new head.
      //      if (freelist_->get(cls) == (ListSizeT)WAP_POINTER_DIFF(block, buffer_)) {
      if ((BlockSizeT)freelist_->get(cls) == offset) {
         // If the new head is null, clear the bitmap.
         if (!next) {
            freelist_->remove(cls);
         } else {
            freelist_->set(cls, block->next_free);
         }
      }
   }

   // Insert a given block into the free list.
   inline void insert(block_header* block, BlockSizeT offset) {
      const SizeClass cls = FreeList::pool_size((ListSizeT)block->get_size());
      const auto current_offset = freelist_->get(cls);
      if (!is_block_inbounds(current_offset)) {
         block->next_free = 0;
         block->prev_free = 0;
         freelist_->set(cls, (ListSizeT)offset);
      } else {
         const auto current = deref_unsafe(current_offset);
         block->next_free = current_offset;
         block->prev_free = 0;
         current->prev_free = offset;
         // Insert the new block at the head of the list.
         freelist_->set(cls, (ListSizeT)offset);
      }
   }

   explicit arena(u8* buffer, BlockSizeT tail, BlockSizeT capacity, FreeList* freelist)
       : buffer_(buffer), capacity_(capacity), tail_(tail), freelist_(freelist) {}

   inline alloc_result append_internal(const BlockSizeT size, const SizeClass cls) {
      auto block = deref_unsafe(tail_);
      auto block_offset = tail_;

      // Reserve by the size class size to promote better free list cache hits
      // if/when deallocated.
      auto remaining_offset = block_offset + cls.size + block_header::OVERHEAD;
      auto remaining = deref_unsafe(remaining_offset);

      //      WAP_ASSERT(remaining && "remaining should not be null");

      // Calculate the remaining size.
      const ListSizeT remain_size = block->get_size() - (block_header::OVERHEAD + cls.size);
      //      WAP_ASSERT(block->size() == remain_size + block_header::OVERHEAD + cls.size);

      // Set the sizes.
      remaining->set_size(remain_size);
      block->set_size(cls.size);
      block->set_used();

      // Link the block to the next block, first.
      //        remaining->prev_phys = block_offset;
      remaining->set_free();
      remaining->set_prev_used();

      // Insert remaining into free list.
      const auto remaining_class = FreeList::pool_size(remain_size);

      // Insert into free list
      auto next_offset = (BlockSizeT)freelist_->get(remaining_class);
      if (WAP_UNLIKELY(next_offset != block_offset)) {
         auto next = deref_safe(next_offset);  // get_head(remaining_class);
         if (!next) {
            remaining->next_free = 0;
            remaining->prev_free = 0;
         } else {
            remaining->next_free = next_offset;
            remaining->prev_free = 0;
            next->prev_free = remaining_offset;
         }
      }

      // Insert the new block at the head of the list.
      freelist_->set(remaining_class, remaining_offset);
      tail_ = remaining_offset;

      // increase size
      set_size(remaining_offset + block_header::OVERHEAD);

      return {block->to_ptr(), block_offset + block_header::DATA_OFFSET, block->get_size()};
   }

   inline BlockSizeT get_head(SizeClass cls) { return (BlockSizeT)freelist_->get(cls); }

   inline void set_size(BlockSizeT size) noexcept { ::wap::bits::le::put_u32(buffer_, size); }

 public:
   void print(void* user = nullptr) { walker::print(buffer_, user); }

   inline bool grow(BlockSizeT by) {
      if (WAP_UNLIKELY(!tail_ || !by)) return false;
      const size_t new_capacity = GrowthPolicy::calc(this->capacity_, by);
      if (WAP_UNLIKELY(new_capacity < (size_t)this->capacity_)) return false;
      const auto new_buffer = Allocator::reallocate(buffer_, new_capacity);
      if (WAP_UNLIKELY(!new_buffer)) return false;

      auto tail = (block_header*)WAP_POINTER_OFFSET(buffer_, tail_);

      if (WAP_UNLIKELY(!tail->is_free())) {
         buffer_ = new_buffer;
         this->capacity_ = (BlockSizeT)new_capacity;
         this->tail_ += tail->get_size();

         tail = (block_header*)WAP_POINTER_OFFSET(buffer_, tail_);
         tail->set_size(by);
         tail->set_prev_free();
         tail->set_prev_used();
         tail->prev_free = 0;
         tail->next_free = 0;

         // Insert into freelist.
         insert(tail, tail_);

         return true;
      }

      const SizeClass previous_class = FreeList::pool_size(tail->get_size());
      const SizeClass new_class = FreeList::pool_size(tail->get_size() + by);

      if (previous_class.index != new_class.index) {
         remove(tail, tail_);
         tail->size += by;
         insert(tail, tail_);
      } else {
         tail->size += by;
      }

      return true;
   }

   // Calculates the smallest size to encompass all used blocks.
   BlockSizeT debug_get_trimmed_size() {
      if (!is_block_inbounds(tail_)) return this->capacity_;
      const auto& tail = *(block_header*)WAP_POINTER_OFFSET(buffer_, tail_);
      if (WAP_LIKELY(tail.is_free())) {
         // If the last block is free then just return the offset to the last block.
         return tail_ + block_header::OVERHEAD;
      }
      // Return the offset to encompass the last size, but NOT the available size.
      return tail_ + block_header::DATA_OFFSET + tail.get_size();
   }

   // Always allocates from the last free block. If the last block is not free or
   // cannot accommodate the size, then the underlying buffer needs to grow.
   alloc_result append(BlockSizeT size) {
      const SizeClass cls = FreeList::alloc_size((ListSizeT)size);

      // Is the size too big?
      if (WAP_UNLIKELY(!cls.size && !grow(size + block_header::OVERHEAD))) {
         return {nullptr, 0, 0};
      }

      auto block = deref_unsafe(tail_);
      if (WAP_UNLIKELY((
             !block->is_free()
             || block->get_size() - size - block_header::DATA_OFFSET < FreeList::get_min_split()))
          && !grow(size + block_header::OVERHEAD)) {
         return {nullptr, 0, 0};
      }

      return append_internal(size, cls);
   }

   alloc_result allocate(const BlockSizeT size) {
      const SizeClass cls = FreeList::alloc_size((ListSizeT)size);

      // Is the size too big?
      if (WAP_UNLIKELY(!cls.size)) return {nullptr, 0, 0};

      // Find a free block that can fit the required size.
      const SizeClass free_class = freelist_->search_class(cls);

      // If not found, then try to extend.
      if (WAP_UNLIKELY(!free_class.size)) {
         // TODO: Be smarter about growth factor
         if (WAP_UNLIKELY(!grow(this->capacity_))) return {nullptr, 0, 0};
         return append_internal(size, cls);
      }

      //      WAP_ASSERT(free_class.size >= cls.size
      //                 && "search_class returned a smaller class than requested");

      // Get the block.
      auto block_offset = (BlockSizeT)freelist_->get(free_class);
      block_header* block = deref_safe(block_offset);
      //      WAP_ASSERT(block_offset > 0 && "freelist is corrupted");
      //      WAP_ASSERT(block && "block should exist");
      //      WAP_ASSERT(block->is_free() && "block must be free");

      if (block_offset == tail_) {
         return append_internal(size, cls);
      }

      // Remove block from free list.
      block_header* prev = block->prev_free ? deref_safe(block->prev_free) : nullptr;
      block_header* next = block->next_free ? deref_safe(block->next_free) : nullptr;
      if (next) {
         if (!prev) {
            next->prev_free = 0;
         } else {
            next->prev_free = block->prev_free;
         }
         freelist_->set(free_class, block->next_free);
      } else {
         freelist_->remove(free_class);
      }
      if (prev) {
         if (next) {
            prev->next_free = block->next_free;
         } else {
            prev->next_free = 0;
         }
      }

      // Maybe split if the free class size is greater than requested?
      if (WAP_LIKELY(free_class.size > cls.size
                     && block->get_size() - cls.size >= FreeList::get_min_split())) {
         // Reserve by the size class size to promote better free list cache hits
         // if/when deallocated.
         auto remaining_offset = block_offset + cls.size + block_header::OVERHEAD;
         auto remaining = deref_unsafe(remaining_offset);

         WAP_ASSERT(remaining && "remaining should not be null");

         // Calculate the remaining size.
         const ListSizeT remain_size = block->get_size() - (block_header::OVERHEAD + cls.size);
         WAP_ASSERT(block->get_size() == remain_size + block_header::OVERHEAD + cls.size);

         // Set the sizes.
         remaining->set_size(remain_size);
         block->set_size(cls.size);

         // Link the block to the next block, first.
         //        remaining->prev_phys = block_offset;
         remaining->set_free();
         remaining->set_prev_used();

         // Insert remaining into free list.
         const SizeClass remaining_class = FreeList::pool_size(remain_size);

         // Insert into free list
         auto next_offset = (BlockSizeT)freelist_->get(remaining_class);
         next = deref_safe(next_offset);  // get_head(remaining_class);
         if (!next) {
            remaining->next_free = 0;
            remaining->prev_free = 0;
         } else {
            remaining->next_free = next_offset;
            remaining->prev_free = 0;
            next->prev_free = remaining_offset;
         }

         // Insert the new block at the head of the list.
         if (remaining_class.index == free_class.index) {
            freelist_->set(remaining_class, remaining_offset);
         } else {
            remove(block, block_offset);
            insert(remaining, remaining_offset);
         }

         if (block_offset == tail_) {
            tail_ = remaining_offset;
            set_size(remaining_offset + block_header::OVERHEAD);
         }
      } else {
         // Get the next adjacent block.
         next = deref_safe(block_offset + block->get_size() + block_header::OVERHEAD);
         if (next) {
            next->set_prev_used();
         }
      }

      // Mark as used and return the pointer to the data.
      block->set_used();
      return {block->to_ptr(), block_offset + block_header::DATA_OFFSET, block->get_size()};
   }

   void deallocate(const BlockSizeT offset) {
      if (!WAP_UNLIKELY(offset)) return;
      const auto block_offset = offset - block_header::DATA_OFFSET;
      block_header* block = deref_safe(block_offset);

      //      WAP_ASSERT(!block->is_free() && "block already marked as free");

      // Get next adjacent block.
      auto next_offset = offset + block->get_size() - block_header::OVERHEAD;
      if (is_block_inbounds(next_offset)) {
         auto next = deref_unsafe(next_offset);

         // Can we merge with next?
         if (next->is_free()) {
            // Remove the next block from free list.
            remove(next, next_offset);
            // Increase size.
            block->size += next->get_size() + block_header::OVERHEAD;

            if (next_offset == tail_) {
               tail_ = block_offset;
               set_size(block_offset + block_header::OVERHEAD);
            } else {
               next_offset = offset + block->get_size();
               if (WAP_LIKELY(is_block_inbounds(next_offset))) {
                  next = deref_unsafe(next_offset);
                  next->prev_phys = block_offset;
               }
            }
         } else {
            next->prev_phys = offset - block_header::DATA_OFFSET;
            next->set_prev_free();
         }
      }

      // Merge with previous physical block?
      if (block->is_prev_free()) {
         block_header* prev = deref_safe(block->prev_phys);
         WAP_ASSERT(prev && prev->is_free() && "prev block is not free though marked as such");
         remove(prev, block->prev_phys);

         if (block_offset == tail_) {
            tail_ = block->prev_phys;
            set_size(block->prev_phys + block_header::OVERHEAD);
         } else {
            // Link next physical block to new previous block.
            next_offset = block->prev_phys + block_header::DATA_OFFSET + prev->get_size();
            if (WAP_LIKELY(is_block_inbounds(next_offset))) {
               block_header* next = deref_unsafe(next_offset);
               next->prev_phys = block->prev_phys;
            }
         }
         prev->size += block->get_size() + block_header::OVERHEAD;
         block = prev;
      } else {
         block->set_free();
      }

      // Insert block back into free list.
      insert(block, block_offset);
   }

   // convenience method that takes a raw pointer and converts to voffset_t
   void free(u8* ptr) {
      const auto offset = WAP_POINTER_DIFF(ptr, buffer_);
      if (WAP_UNLIKELY(offset < 1 || offset > this->tail_ + block_header::DATA_OFFSET)) return;
      deallocate((voffset_t)offset);
   }

   // Reallocate / resize an existing allocation.
   alloc_result reallocate(const BlockSizeT offset, const BlockSizeT size) {
      auto block_offset = offset - block_header::DATA_OFFSET;
      auto block = deref_safe(block_offset);
      if (WAP_UNLIKELY(!block)) {
         return {nullptr, 0, 0};
      }

      WAP_ASSERT(!block->is_free() && "block already marked as free");

      // Shrink?
      if (WAP_UNLIKELY(size < block->get_size())) {
         return {block->to_ptr(), offset, size};
      }

      auto extend_size = size - block->get_size();
      // Already a perfect fit?
      if (WAP_UNLIKELY(!extend_size)) {
         return {block->to_ptr(), offset, size};
      }

      // Get next adjacent block.
      auto next_offset = offset + block->get_size() - block_header::OVERHEAD;
      if (WAP_LIKELY(is_block_inbounds(next_offset))) {
         auto next = deref_unsafe(next_offset);

         // Can with the next adjacent block?
         if (next->is_free()) {
            const auto next_size = next->get_size() + block_header::OVERHEAD;

            // Split?
            if (WAP_LIKELY(
                   next_size >= extend_size + FreeList::get_min_size() + block_header::OVERHEAD)) {
               // Remove from freelist.
               remove(next, next_offset);

               // Reserve by the size class size to promote better free list cache hits
               // if/when deallocated.
               auto remaining_offset = offset + extend_size + block_header::OVERHEAD;
               auto remaining = deref_unsafe(remaining_offset);

               WAP_ASSERT(remaining && "remaining should not be null");

               // Calculate the remaining size.
               const ListSizeT remain_size
                  = block->get_size() - (block_header::OVERHEAD + extend_size);
               WAP_ASSERT(block->get_size() == remain_size + block_header::OVERHEAD + extend_size);

               // Set the sizes.
               remaining->set_size(remain_size);
               block->set_size(size);

               // Link the block to the next block, first.
               remaining->set_free();
               remaining->set_prev_used();

               // Insert remaining into free list.
               const auto remaining_class = FreeList::pool_size(remain_size);

               if (next_offset == tail_) {
                  tail_ = remaining_offset;
               }

               // Insert into free list
               next_offset = (BlockSizeT)freelist_->get(remaining_class);
               next = deref_safe(next_offset);  // get_head(remaining_class);
               if (!next) {
                  remaining->next_free = 0;
                  remaining->prev_free = 0;
               } else {
                  remaining->next_free = next_offset;
                  remaining->prev_free = 0;
                  next->prev_free = remaining_offset;
               }
               // Insert the new block at the head of the list.
               freelist_->set(remaining_class, remaining_offset);

               return {block->to_ptr(), offset, size};
            }

            // Consume?
            if (next_size >= extend_size) {
               remove(next, next_offset);

               const auto adjusted_size = block->get_size() + next_size;
            }

            if (block->is_prev_free()) {
            }
         }
      } else if (block->is_prev_free()) {
         block_header* prev = deref_safe(block->prev_phys);
         WAP_ASSERT(prev && prev->is_free() && "prev block is not free though marked as such");
      }

      auto result = allocate(size);
      if (result.ptr) {
         deallocate(offset);
         return result;
      }
      return {nullptr, 0, 0};
   }

   static arena make(BlockSizeT base_size, BlockSizeT capacity) {
      auto buffer = (u8*)Allocator::allocate((size_t)capacity);
      memset(buffer, 0, capacity);

      ptrdiff_t base_offset = 0;  //-4;  // sizeof(uint32_t);
      //      auto& base_block = *(block_header*)WAP_POINTER_OFFSET(buffer, (ptrdiff_t)base_offset
      //      - (ptrdiff_t)block_header::OVERHEAD);
      auto& base_block = *(block_header*)WAP_POINTER_OFFSET(buffer, (ptrdiff_t)base_offset);

      // put base_block capacity
      base_block.set_size(base_size);
      base_block.set_used();

      //      auto after_base = (u8*)WAP_POINTER_OFFSET(
      //          buffer, base_size + (block_header::OVERHEAD + sizeof(SizeT)));

      // Setup / allocate freelist.
      const ptrdiff_t freelist_offset
         = base_offset + base_block.get_size() + block_header::OVERHEAD;
      auto& freelist_block = *(block_header*)WAP_POINTER_OFFSET(buffer, freelist_offset);
      freelist_block.set_size(sizeof(FreeList));
      freelist_block.set_used();
      auto freelist = (FreeList*)freelist_block.to_ptr();

      const ptrdiff_t last_offset
         = freelist_offset + block_header::OVERHEAD + freelist_block.get_size();
      const auto last = (block_header*)WAP_POINTER_OFFSET(buffer, last_offset);
      auto last_offset_check = WAP_POINTER_DIFF(last, buffer);
      (void)last_offset_check;
      ListSizeT pool_bytes = (ptrdiff_t)capacity - last_offset - (ptrdiff_t)block_header::OVERHEAD;

      const auto cls = FreeList::pool_size(pool_bytes);

      last->set_size(pool_bytes);
      last->set_free();
      last->next_free = 0;
      last->prev_free = 0;
      freelist->set(cls, (ListSizeT)(last_offset));

      ::wap::bits::le::put_u32(buffer, last_offset + block_header::OVERHEAD);

      return arena(buffer, last_offset, capacity, freelist);
   }

 private:
   u8* buffer_;
   BlockSizeT capacity_;
   BlockSizeT tail_;
   FreeList* freelist_;
};

template <typename Allocator = ::wap::allocator, typename FreeListT = ::wap::small::free_list>
struct arena_extern_freelist : public arena<Allocator, FreeListT> {
   using SizeT = block_header::SizeT;

 public:
   arena_extern_freelist() : arena_extern_freelist<Allocator, FreeListT>(&freelist_) {}

   FreeListT freelist_{};
};

}  // namespace wap

#undef WAP_SLICE_BIT_FUNCS
#undef WAP_SLICE_FUNCS
#undef WAP_SLICE_SAFE_FUNCS
#undef WAP_SLICE_BIT_BOUNDS_CHECK_FUNCS

#endif  // WAP_ALLOC_H
