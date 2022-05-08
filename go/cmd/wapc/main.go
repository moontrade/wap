package main

import (
	"fmt"
	"github.com/moontrade/wap/go/memory"
	"github.com/moontrade/wap/go/schema/parser"
	"os"
)

func main() {
	defer func() {
		if err := recover(); err != nil {
			fmt.Println(err)
		}
	}()
	a := memory.Alloc(32)
	a.Free()
	if len(os.Args) < 2 {
		fmt.Println("expected at least 1 file or directory")
		return
	}
	data, err := os.ReadFile(os.Args[1])
	if err != nil {
		fmt.Println(err.Error())
		return
	}
	p := parser.NewParser(data)

	err = p.Parse()
	if err != nil {
		fmt.Println(err.Error())
	}
}
