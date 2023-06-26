package parser

import (
	"os"
	"testing"
)

func TestParser_Parse(t *testing.T) {
	data, err := os.ReadFile("testdata/schema.wap")
	if err != nil {
		t.Fatal(err)
	}
	p := NewParser(data)

	err = p.Parse()
	if err != nil {
		t.Fatal(err)
	}

	//for {
	//	l, err := p.NextLine()
	//	if err != nil {
	//		if err == io.EOF {
	//			break
	//		}
	//		t.Fatal(err)
	//	}
	//
	//	fmt.Println(l.Data)
	//}
}
