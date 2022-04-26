package parser

import (
	"fmt"
	"github.com/moontrade/wap/memory"
	"runtime"
	"testing"
	"unsafe"
)

//func isEmpty[B string | []byte](p B) bool {
//	return len(p) > 0
//}
//
//type T1 struct {
//	hi func()
//}
//
//func (T1) Hi() {
//	fmt.Println("T1: Hi")
//}
//
//type T2 struct {
//	hi func()
//}
//
//func (T2) Hi() {
//	fmt.Println("T2: Hi")
//}
//
//func CallHi[B T1 | T2](p B) {
//	(*(*func())(unsafe.Pointer(&p)))()
//}
//
//func TestHi(t *testing.T) {
//	var (
//		t1 T1
//		t2 T2
//	)
//	t1.hi = t1.Hi
//	t2.hi = t2.Hi
//	CallHi(t1)
//	CallHi(t2)
//}

func TestGC(t *testing.T) {
	{
		p := memory.AllocZeroed(32)
		h := (*Order)(p.Unsafe())
		h.SetPrice(99.99)
		fmt.Println(h.Price())
	}
	{
		runtime.GC()
		p := memory.GCAlloc(32)
		h := (*Order)(p)
		h.SetPrice(99.99)
		fmt.Println(h.Price())
	}
}

func TestAlloc(t *testing.T) {
	{
		p := memory.AllocZeroed(32)
		h := (*Order)(p.Unsafe())
		h.SetPrice(99.99)
		fmt.Println(h.Price())
	}
	{
		p := memory.GCAllocZeroed(32)
		h := (*Order)(p)
		h.SetPrice(99.99)
		fmt.Println(h.Price())
	}
}

func BenchmarkNewOrder(b *testing.B) {
	b.Run("Raw", func(b *testing.B) {
		o := &OrderStruct{}
		b.ReportAllocs()
		b.ResetTimer()
		for i := 0; i < b.N; i++ {
			o.SetID(int32(i))
			o.SetID(int32(i))
			o.SetID(int32(i))
			o.SetID(int32(i))
			o.ID()
			o.ID()
			o.ID()
			o.ID()
		}
	})

	b.Run("Heap", func(b *testing.B) {
		o := NewOrderHeap()
		b.ReportAllocs()
		b.ResetTimer()
		for i := 0; i < b.N; i++ {
			o.SetID(int32(i))
			o.SetID(int32(i))
			o.SetID(int32(i))
			o.SetID(int32(i))
			o.ID()
			o.ID()
			o.ID()
			o.ID()
		}
	})

	b.Run("Untrusted Reader", func(b *testing.B) {
		h := NewOrderHeap()
		o := NewOrderReader(h, uintptr(unsafe.Add(unsafe.Pointer(h), 8)))
		b.ReportAllocs()
		b.ResetTimer()
		for i := 0; i < b.N; i++ {
			o.SetID(int32(i))
			o.SetID(int32(i))
			o.SetID(int32(i))
			o.SetID(int32(i))
			o.ID()
			o.ID()
			o.ID()
			o.ID()
		}
	})

	b.Run("Builder", func(b *testing.B) {
		h := NewOrderHeap()
		o := NewOrderBuilder(&h)
		b.ReportAllocs()
		b.ResetTimer()
		for i := 0; i < b.N; i++ {
			o.SetID(int32(i))
			o.SetID(int32(i))
			o.SetID(int32(i))
			o.SetID(int32(i))
			o.ID()
			o.ID()
			o.ID()
			o.ID()
		}
	})
}
