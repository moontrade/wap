package parser

import (
	"fmt"
	"testing"
	"unsafe"
)

func TestOrder_ID(t *testing.T) {
	o := Order[Trusted]{}
	o2 := Order[TrustedView]{}
	_ = o
	_ = o2
	fmt.Println(unsafe.Sizeof(Order[Trusted]{}))
	fmt.Println(unsafe.Sizeof(Order[TrustedView]{}))
}

//func Sizeof()
