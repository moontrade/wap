package pb

import (
	"fmt"
	"testing"
)

func TestWrite(t *testing.T) {
	b := NewBuffer(make([]byte, 0, 4096))
	b.WriteUvarint32(1, 2)
	b.WriteFixed32Float32(2, 3.1)
	b.WriteZigzag32Int32(3, -1)
	//b.WriteVarint32(3, -1)

	fmt.Println("Encoded Length", len(b.Bytes()))

	var (
		f   Number
		err error
		i32 int32
		f32 float32
	)
	for !b.EOF() && err == nil {
		f, _, err = b.DecodeTag()
		if err != nil {
			break
		}
		switch f {
		case 1:
			i32 = b.ReadInt32()
			fmt.Println("Field 1 = ", i32)
		case 2:
			f32 = b.ReadFloat32()
			fmt.Println("Field 2 = ", f32)
		case 3:
			i32 = b.ReadInt32Zigzag()
			//i32, err = b.ReadInt32()
			fmt.Println("Field 3 = ", i32)
		default:
			b.Consume()
			err = b.err
		}
	}
	if err != nil {
		t.Fatal(err)
	}
}
