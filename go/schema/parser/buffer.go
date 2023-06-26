package parser

import (
	"github.com/moontrade/wap/go/memory"
	"math/bits"
	"unsafe"
)

type OrderStruct struct {
	id    int32
	price float32
}

func (o *OrderStruct) ID() int32 {
	return o.id
}
func (o *OrderStruct) Price() float32 {
	return o.price
}
func (o *OrderStruct) SetID(v int32) {
	o.id = v
}
func (o *OrderStruct) SetPrice(v float32) {
	o.price = v
}

type Order struct {
	id    [4]byte
	price [4]byte
}

func (o *Order) AsReader() OrderReader {
	return OrderReader{o: o, end: uintptr(unsafe.Add(unsafe.Pointer(o), unsafe.Sizeof(Order{})))}
}

func (o *Order) ID() int32 {
	return *(*int32)(unsafe.Pointer(o))
}

func (o *Order) IDbe() int32 {
	return int32(bits.Reverse32(*(*uint32)(unsafe.Pointer(o))))
}

func (o *Order) Price() float32 {
	return *(*float32)(unsafe.Add(unsafe.Pointer(o), 4))
}

func (o *Order) SetID(v int32) {
	*(*int32)(unsafe.Pointer(&o)) = v
}

func (o *Order) SetPrice(v float32) {
	*(*float32)(unsafe.Add(unsafe.Pointer(o), 4)) = v
}

func NewOrderHeap() *Order {
	p := memory.AllocZeroed(16)
	return (*Order)(p.Unsafe())
}

type OrderHeapContainer struct {
	O *Order
	_ int64
}

func (o *OrderHeapContainer) ID() int32 {
	return *(*int32)(unsafe.Pointer(o.O))
}

type OrderStructContainer struct {
	O *OrderStruct
	_ int64
	_ int64
}

func (o *OrderStructContainer) ID() int32 {
	return o.O.ID()
}

type OrderReader struct {
	o   *Order
	end uintptr
}

func NewOrderReader(o *Order, end uintptr) OrderReader {
	return OrderReader{o, end}
}

func (o OrderReader) Unsafe() *Order {
	return o.o
}

func (o OrderReader) ID() int32 {
	if uintptr(unsafe.Add(unsafe.Pointer(o.o), 4)) > o.end {
		return 0
	}
	return o.o.ID()
}

func (o OrderReader) SetID(v int32) {
	if uintptr(unsafe.Add(unsafe.Pointer(o.o), 4)) > o.end {
		return
	}
	o.o.SetID(v)
}

type OrderBuilder struct {
	o **Order
}

func NewOrderBuilder(o **Order) OrderBuilder {
	return OrderBuilder{o}
}

func (o OrderBuilder) ID() int32 {
	//if uintptr(unsafe.Add(unsafe.Pointer(*o.o), 4)) > o.end {
	//	return 0
	//}
	return (*o.o).ID()
}

func (o OrderBuilder) SetID(v int32) {
	//if uintptr(unsafe.Add(unsafe.Pointer(o.o), 4)) > o.end {
	//	return
	//}
	(*o.o).SetID(v)
}
