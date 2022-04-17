package parser

import (
	"github.com/moontrade/wap/memory"
	"unsafe"
)

type View interface {
	TrustedView

	I32(o int) int32
}

type Slice interface {
	Trusted |
		TrustedView

	I32(o int) int32
}

type TrustedView struct {
	d unsafe.Pointer
	Trusted
}

func (b TrustedView) I32(o int) int32 {
	return b.b.Int32LE(o)
}

type Trusted struct {
	b memory.Pointer
}

func (b Trusted) I32(o int) int32 {
	return b.b.Int32LE(o)
}

type Untrusted struct {
	b   memory.Pointer
	end memory.Pointer
}

func (b Untrusted) I32(o int) int32 {
	if b.b.Add(o) > b.end-4 {
		return 0
	}
	return b.b.Int32LE(o)
}

type Root[V View, T any] struct {
	v     V
	Value T
}

type Order[T Slice] struct {
	t T
}

func (o *Order[T]) ID() int32 {
	return o.t.I32(0)
}
