package main

import (
	"github.com/moontrade/wap/internal/unsafecgo"
)

func main() {
	//cgo.CGO()
	unsafecgo.NonBlocking((*byte)(nil), 0, 0)
}
