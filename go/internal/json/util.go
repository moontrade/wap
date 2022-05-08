package json

import (
	"unsafe"
)

type _string struct {
	Data uintptr
	Len  int
}

type _slice struct {
	Data uintptr
	Len  int
	Cap  int
}

// bytesToStr creates a string pointing at the slice to avoid copying.
//
// Warning: the string returned by the function should be used with care, as the whole input data
// chunk may be either blocked from being freed by GC because of a single string or the buffer.Data
// may be garbage-collected even when the string exists.
func bytesToStr(data []byte) string {
	h := (*_slice)(unsafe.Pointer(&data))
	return *(*string)(unsafe.Pointer(&_string{Data: h.Data, Len: h.Len}))
}
