package parser

import "encoding/binary"

type Comment struct {
	Star   bool
	Inline bool
	Lines  []CommentLine
}

type CommentLine struct {
	Line Line
	Text string
}

type TypeCode byte

const (
	TypeUnknown        TypeCode = 0
	TypeCodeBool       TypeCode = 1
	TypeCodeI8         TypeCode = 2
	TypeCodeU8         TypeCode = 3
	TypeCodeI16        TypeCode = 4
	TypeCodeU16        TypeCode = 5
	TypeCodeI32        TypeCode = 6
	TypeCodeU32        TypeCode = 7
	TypeCodeI64        TypeCode = 8
	TypeCodeU64        TypeCode = 9
	TypeCodeI128       TypeCode = 10
	TypeCodeU128       TypeCode = 11
	TypeCodeF32        TypeCode = 12
	TypeCodeF64        TypeCode = 13
	TypeCodeF128       TypeCode = 14
	TypeString         TypeCode = 20
	TypeStringInline   TypeCode = 21
	TypeCodeEnum       TypeCode = 25
	TypeCodeStruct     TypeCode = 30
	TypeCodeUnion      TypeCode = 31
	TypeCodeVariant    TypeCode = 32
	TypeCodePointer    TypeCode = 33
	TypeCodeArray      TypeCode = 40
	TypeCodeVector     TypeCode = 41
	TypeCodeMap        TypeCode = 50 // robin-hood map
	TypeCodeMapOrdered TypeCode = 51 // ART radix tree
	TypeCodeMapTree    TypeCode = 52 // B+Tree
	TypeCodeSet        TypeCode = 60
	TypeCodeSetOrdered TypeCode = 61
	TypeCodeSetTree    TypeCode = 62
	TypeCodePad        TypeCode = 100
)

type Endian byte

const (
	EndianLittle Endian = 0
	EndianBig    Endian = 1
	EndianNative Endian = 2
)

type Number struct {
	Code   TypeCode
	Endian Endian
}

func (n *Number) Size() int {
	switch n.Code {
	case TypeCodeI8, TypeCodeU8:
		return 1
	case TypeCodeI16, TypeCodeU16:
		return 2
	case TypeCodeI32, TypeCodeU32, TypeCodeF32:
		return 4
	case TypeCodeI64, TypeCodeU64, TypeCodeF64:
		return 8
	}
	return 0
}

func (n *Number) ToValue(v uint64) *NumberValue {
	value := &NumberValue{
		Number: n,
	}
	value.Set(v)
	return value
}

func (n *Number) ToValueSigned(v int64) *NumberValue {
	value := &NumberValue{
		Number: n,
	}
	value.Set(uint64(v))
	return value
}

var (
	Bool = Type{Name: "bool", Code: TypeCodeBool, Number: &Number{Code: TypeCodeBool}}
	I8   = Type{Name: "i8", Code: TypeCodeI8, Number: &Number{Code: TypeCodeI8}}
	I16  = Type{Name: "i16", Code: TypeCodeI16, Number: &Number{TypeCodeI16, EndianLittle}}
	I16B = Type{Name: "i16b", Code: TypeCodeI16, Number: &Number{TypeCodeI16, EndianBig}}
	I16N = Type{Name: "i16n", Code: TypeCodeI16, Number: &Number{TypeCodeI16, EndianNative}}
	I32  = Type{Name: "i32", Code: TypeCodeI32, Number: &Number{TypeCodeI32, EndianLittle}}
	I32B = Type{Name: "i32b", Code: TypeCodeI32, Number: &Number{TypeCodeI32, EndianBig}}
	I32N = Type{Name: "i32n", Code: TypeCodeI32, Number: &Number{TypeCodeI32, EndianNative}}
	I64  = Type{Name: "i64", Code: TypeCodeI64, Number: &Number{TypeCodeI64, EndianLittle}}
	I64B = Type{Name: "i64b", Code: TypeCodeI64, Number: &Number{TypeCodeI64, EndianBig}}
	I64N = Type{Name: "i64n", Code: TypeCodeI64, Number: &Number{TypeCodeI64, EndianNative}}
	U8   = Type{Name: "u8", Code: TypeCodeU8, Number: &Number{Code: TypeCodeU8}}
	U16  = Type{Name: "u16", Code: TypeCodeU16, Number: &Number{TypeCodeU16, EndianLittle}}
	U16B = Type{Name: "u16b", Code: TypeCodeU16, Number: &Number{TypeCodeU16, EndianBig}}
	U16N = Type{Name: "u16n", Code: TypeCodeU16, Number: &Number{TypeCodeU16, EndianNative}}
	U32  = Type{Name: "u32", Code: TypeCodeU32, Number: &Number{TypeCodeU32, EndianLittle}}
	U32B = Type{Name: "u32b", Code: TypeCodeU32, Number: &Number{TypeCodeU32, EndianBig}}
	U32N = Type{Name: "u32n", Code: TypeCodeU32, Number: &Number{TypeCodeU32, EndianNative}}
	U64  = Type{Name: "u64", Code: TypeCodeU64, Number: &Number{TypeCodeU64, EndianLittle}}
	U64B = Type{Name: "u64b", Code: TypeCodeU64, Number: &Number{TypeCodeU64, EndianBig}}
	U64N = Type{Name: "u64n", Code: TypeCodeU64, Number: &Number{TypeCodeU64, EndianNative}}
	F32  = Type{Name: "f32", Code: TypeCodeF32, Number: &Number{TypeCodeF64, EndianLittle}}
	F32B = Type{Name: "f32b", Code: TypeCodeF32, Number: &Number{TypeCodeF64, EndianBig}}
	F32N = Type{Name: "f32n", Code: TypeCodeF32, Number: &Number{TypeCodeF64, EndianNative}}
	F64  = Type{Name: "f64", Code: TypeCodeF64, Number: &Number{TypeCodeF64, EndianLittle}}
	F64B = Type{Name: "f64b", Code: TypeCodeF64, Number: &Number{TypeCodeF64, EndianBig}}
	F64N = Type{Name: "f64n", Code: TypeCodeF64, Number: &Number{TypeCodeF64, EndianNative}}
)

type NumberValue struct {
	*Number
	value [8]byte
}

func (nv *NumberValue) Size() int {
	switch nv.Number.Code {
	case TypeCodeBool, TypeCodeI8, TypeCodeU8:
		return 1
	case TypeCodeI16, TypeCodeU16:
		return 2
	case TypeCodeI32, TypeCodeU32, TypeCodeF32:
		return 4
	case TypeCodeI64, TypeCodeU64, TypeCodeF64:
		return 8
	}
	return 0
}

/*

 */

func (nv *NumberValue) Get() uint64 {
	switch nv.Number.Code {
	case TypeCodeBool, TypeCodeI8, TypeCodeU8:
		return uint64(nv.value[0])
	case TypeCodeI16, TypeCodeU16:
		if nv.Number.Endian == EndianBig {
			return uint64(binary.BigEndian.Uint16(nv.value[0:2]))
		} else {
			return uint64(binary.LittleEndian.Uint16(nv.value[0:2]))
		}
	case TypeCodeI32, TypeCodeU32, TypeCodeF32:
		if nv.Number.Endian == EndianBig {
			return uint64(binary.BigEndian.Uint32(nv.value[0:4]))
		} else {
			return uint64(binary.LittleEndian.Uint32(nv.value[0:4]))
		}
	case TypeCodeI64, TypeCodeU64, TypeCodeF64:
		if nv.Number.Endian == EndianBig {
			return binary.BigEndian.Uint64(nv.value[0:8])
		} else {
			return binary.LittleEndian.Uint64(nv.value[0:8])
		}
	}
	return 0
}

func (nv *NumberValue) Append(b []byte) []byte {
	switch nv.Number.Code {
	case TypeCodeBool, TypeCodeI8, TypeCodeU8:
		b = append(b, nv.value[0])
	case TypeCodeI16, TypeCodeU16:
		b = append(b, nv.value[0:2]...)
	case TypeCodeI32, TypeCodeU32, TypeCodeF32:
		b = append(b, nv.value[0:4]...)
	case TypeCodeI64, TypeCodeU64, TypeCodeF64:
		b = append(b, nv.value[0:8]...)
	}
	return b
}

func (nv *NumberValue) Set(value uint64) {
	switch nv.Number.Code {
	case TypeCodeBool, TypeCodeI8, TypeCodeU8:
		nv.value[0] = byte(value)
	case TypeCodeI16, TypeCodeU16:
		if nv.Number.Endian == EndianBig {
			binary.BigEndian.PutUint16(nv.value[0:2], uint16(value))
		} else {
			binary.LittleEndian.PutUint16(nv.value[0:2], uint16(value))
		}
	case TypeCodeI32, TypeCodeU32, TypeCodeF32:
		if nv.Number.Endian == EndianBig {
			binary.BigEndian.PutUint32(nv.value[0:4], uint32(value))
		} else {
			binary.LittleEndian.PutUint32(nv.value[0:4], uint32(value))
		}
	case TypeCodeI64, TypeCodeU64, TypeCodeF64:
		if nv.Number.Endian == EndianBig {
			binary.BigEndian.PutUint64(nv.value[0:8], value)
		} else {
			binary.LittleEndian.PutUint64(nv.value[0:8], value)
		}
	}
}

// Type variant
type Type struct {
	Parent    *Type
	Name      string
	StartLine Line
	EndLine   Line
	Comments  []*Comment
	Pointer   *Pointer
	Number    *Number
	Enum      *Enum
	Field     *Field
	Struct    *Struct
	Alias     *Alias
	Variant   *Variant
	Value     *Value
	Vector    *Vector
	Map       *Map
	Code      TypeCode
}

func (t *Type) Size() int {
	switch {
	case t.Pointer != nil:
		return t.Pointer.Size
	case t.Number != nil:
		return t.Number.Size()
	case t.Enum != nil:
		return t.Enum.ValueType.Size()
	case t.Alias != nil:
		return t.Alias.Computed.Size()
	case t.Struct != nil:
		return t.Struct.Size
	case t.Variant != nil:

	}
	return 0
}

func (lhs *Type) Equal(rhs *Type) {

}

type Pointer struct {
	Type *Type
	Size int
}

type Value struct {
	Type    *Type
	Const   *Const
	Enum    *EnumOption
	Variant *VariantOption
	Struct  *StructValue
	Number  *NumberValue
}

func (v *Value) Append(b []byte) []byte {
	switch {
	case v.Const != nil:
		b = v.Const.Value.Append(b)
	case v.Enum != nil:
	case v.Variant != nil:
	case v.Struct != nil:
		b = v.Struct.Append(b)
	case v.Number != nil:
		b = v.Number.Append(b)
	}
	return b
}

type Const struct {
	Name  string
	Value Value
}

type Attribute struct {
	Name string
	Args []string
}

type AlignAttribute struct {
	Value int
}

type PackAttribute struct {
	Value int
}

type Alias struct {
	Type     Type
	Computed *Type
}

type Struct struct {
	Parent *Struct
	Type   *Type
	Inline *Field
	Fields []*Field
	Align  int
	Pack   int
	Size   int
	Inner  []*Type
	Union  bool
}

func NewStruct(name string, line Line, comments []*Comment) *Struct {
	s := &Struct{Type: &Type{StartLine: line}}
	s.Type.Struct = s
	s.Type.Code = TypeCodeStruct
	s.Type.Name = name
	s.Type.Comments = comments
	return s
}

func (f *Field) AsUnion() *Struct {
	s := &Struct{Type: &Type{}}
	s.Type.Struct = s
	s.Type.Code = TypeCodeUnion
	s.Union = true
	s.Inline = f
	return s
}

func (s *Struct) NewField(line Line, comments []*Comment) *Field {
	f := &Field{Struct: s, Type: &Type{StartLine: line, Comments: comments}}
	f.Type.Field = f
	s.Fields = append(s.Fields, f)
	return f
}

func (s *Struct) Unaligned() int {
	return 0
}

type Field struct {
	Struct         *Struct
	Name           string
	Number         int // 1 based field index
	NumberDeclared bool
	JsonNames      []string // Json names to match
	ProtoNumber    int      // Protobuf field number. Defaults to Number
	Type           *Type
	Size           int
	Offset         int
	Initial        Value
}

type StructValue struct {
	Fields []FieldValue
}

func (sv *StructValue) Append(b []byte) []byte {
	return b
}

type FieldValue struct {
	Field *Field
	Value Value
}

type Variant struct {
	Type    *Type
	Inline  *Field
	Options []VariantOption
}

type VariantOption struct {
	Variant *Variant
	Type    *Type
}

func (vo *VariantOption) Size() int {
	return vo.Type.Size()
}

type Enum struct {
	Parent    *Type
	Type      Type
	Inline    *Field
	ValueType Number
	Options   []*EnumOption
}

type EnumOption struct {
	Enum     *Enum
	JsonName string
	Value    NumberValue
}

type Map struct {
	Kind  TypeCode
	Key   *Type
	Value *Type
}

type Vector struct {
	Kind    TypeCode
	Element *Type
	Length  int
}

type String struct {
	Code        TypeCode
	PointerSize int
	Inline      int
	Max         int
}

func (s *String) FieldSize() int {
	if s.Inline > 0 {
		return s.Inline
	}
	return s.PointerSize
}

func (s *String) IsPointer() bool {
	return s.Max == 0 || s.Max > s.Inline
}

type Protocol struct {
}

type Import struct {
	Name string
	File File
}

type File struct {
	Constants       []*Const
	ConstantsByName map[string]*Const
	Structs         []*Struct
	Types           map[string]*Type
}