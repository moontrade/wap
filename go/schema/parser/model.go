package parser

import (
	"encoding/binary"
	"math"
)

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
	TypeCodeUnknown      TypeCode = 0
	TypeCodeBool         TypeCode = 1
	TypeCodeI8           TypeCode = 2
	TypeCodeU8           TypeCode = 3
	TypeCodeI16          TypeCode = 4
	TypeCodeU16          TypeCode = 5
	TypeCodeI32          TypeCode = 6
	TypeCodeU32          TypeCode = 7
	TypeCodeI64          TypeCode = 8
	TypeCodeU64          TypeCode = 9
	TypeCodeI128         TypeCode = 10
	TypeCodeU128         TypeCode = 11
	TypeCodeF32          TypeCode = 12
	TypeCodeF64          TypeCode = 13
	TypeCodeF128         TypeCode = 14
	TypeString           TypeCode = 20 // heap allocated string
	TypeStringInline     TypeCode = 21 // string embedded inline
	TypeStringInlinePlus TypeCode = 22 // string embedded inline with a heap allocated spill-over
	TypeCodeEnum         TypeCode = 25
	TypeCodeStruct       TypeCode = 30
	TypeCodeUnion        TypeCode = 31
	TypeCodeVariant      TypeCode = 32
	TypeCodeOptional     TypeCode = 33
	TypeCodePointer      TypeCode = 34
	TypeCodeArray        TypeCode = 40
	TypeCodeVector       TypeCode = 41
	TypeCodeMap          TypeCode = 50 // robin-hood map
	TypeCodeMapOrdered   TypeCode = 51 // ART radix tree
	TypeCodeMapTree      TypeCode = 52 // B+Tree
	TypeCodeSet          TypeCode = 60
	TypeCodeSetOrdered   TypeCode = 61
	TypeCodeSetTree      TypeCode = 62
	TypeCodeConst        TypeCode = 90
	TypeCodePad          TypeCode = 100
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

func MinNumber(code TypeCode) int64 {
	switch code {
	case TypeCodeI8:
		return math.MinInt8
	case TypeCodeI16:
		return math.MinInt16
	case TypeCodeI32:
		return math.MinInt32
	case TypeCodeI64:
		return math.MinInt64
	}
	return 0
}

func MaxNumber(code TypeCode) uint64 {
	switch code {
	case TypeCodeI8:
		return math.MaxInt8
	case TypeCodeI16:
		return math.MaxInt16
	case TypeCodeI32:
		return math.MaxInt32
	case TypeCodeI64:
		return math.MaxInt64

	case TypeCodeU8:
		return math.MaxUint8
	case TypeCodeU16:
		return math.MaxUint16
	case TypeCodeU32:
		return math.MaxUint32
	case TypeCodeU64:
		return math.MaxUint64

	case TypeCodeF32:
		return math.Float64bits(math.MaxFloat32)

	case TypeCodeF64:
		return math.Float64bits(math.MaxFloat64)
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
	Bool  = Type{Name: "bool", Code: TypeCodeBool, Number: &Number{Code: TypeCodeBool}}
	I8    = Type{Name: "i8", Code: TypeCodeI8, Number: &Number{Code: TypeCodeI8}}
	I16   = Type{Name: "i16", Code: TypeCodeI16, Number: &Number{TypeCodeI16, EndianLittle}}
	I16B  = Type{Name: "i16b", Code: TypeCodeI16, Number: &Number{TypeCodeI16, EndianBig}}
	I16N  = Type{Name: "i16n", Code: TypeCodeI16, Number: &Number{TypeCodeI16, EndianNative}}
	I32   = Type{Name: "i32", Code: TypeCodeI32, Number: &Number{TypeCodeI32, EndianLittle}}
	I32B  = Type{Name: "i32b", Code: TypeCodeI32, Number: &Number{TypeCodeI32, EndianBig}}
	I32N  = Type{Name: "i32n", Code: TypeCodeI32, Number: &Number{TypeCodeI32, EndianNative}}
	I64   = Type{Name: "i64", Code: TypeCodeI64, Number: &Number{TypeCodeI64, EndianLittle}}
	I64B  = Type{Name: "i64b", Code: TypeCodeI64, Number: &Number{TypeCodeI64, EndianBig}}
	I64N  = Type{Name: "i64n", Code: TypeCodeI64, Number: &Number{TypeCodeI64, EndianNative}}
	I128  = Type{Name: "i128", Code: TypeCodeI128, Number: &Number{TypeCodeI128, EndianLittle}}
	I128B = Type{Name: "i128b", Code: TypeCodeI128, Number: &Number{TypeCodeI128, EndianBig}}
	I128N = Type{Name: "i128n", Code: TypeCodeI128, Number: &Number{TypeCodeI128, EndianNative}}
	U8    = Type{Name: "u8", Code: TypeCodeU8, Number: &Number{Code: TypeCodeU8}}
	U16   = Type{Name: "u16", Code: TypeCodeU16, Number: &Number{TypeCodeU16, EndianLittle}}
	U16B  = Type{Name: "u16b", Code: TypeCodeU16, Number: &Number{TypeCodeU16, EndianBig}}
	U16N  = Type{Name: "u16n", Code: TypeCodeU16, Number: &Number{TypeCodeU16, EndianNative}}
	U32   = Type{Name: "u32", Code: TypeCodeU32, Number: &Number{TypeCodeU32, EndianLittle}}
	U32B  = Type{Name: "u32b", Code: TypeCodeU32, Number: &Number{TypeCodeU32, EndianBig}}
	U32N  = Type{Name: "u32n", Code: TypeCodeU32, Number: &Number{TypeCodeU32, EndianNative}}
	U64   = Type{Name: "u64", Code: TypeCodeU64, Number: &Number{TypeCodeU64, EndianLittle}}
	U64B  = Type{Name: "u64b", Code: TypeCodeU64, Number: &Number{TypeCodeU64, EndianBig}}
	U64N  = Type{Name: "u64n", Code: TypeCodeU64, Number: &Number{TypeCodeU64, EndianNative}}
	U128  = Type{Name: "u128", Code: TypeCodeU128, Number: &Number{TypeCodeU128, EndianLittle}}
	U128B = Type{Name: "u128b", Code: TypeCodeU128, Number: &Number{TypeCodeU128, EndianBig}}
	U128N = Type{Name: "u128n", Code: TypeCodeU128, Number: &Number{TypeCodeU128, EndianNative}}
	F32   = Type{Name: "f32", Code: TypeCodeF32, Number: &Number{TypeCodeF64, EndianLittle}}
	F32B  = Type{Name: "f32b", Code: TypeCodeF32, Number: &Number{TypeCodeF64, EndianBig}}
	F32N  = Type{Name: "f32n", Code: TypeCodeF32, Number: &Number{TypeCodeF64, EndianNative}}
	F64   = Type{Name: "f64", Code: TypeCodeF64, Number: &Number{TypeCodeF64, EndianLittle}}
	F64B  = Type{Name: "f64b", Code: TypeCodeF64, Number: &Number{TypeCodeF64, EndianBig}}
	F64N  = Type{Name: "f64n", Code: TypeCodeF64, Number: &Number{TypeCodeF64, EndianNative}}
)

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

func (nv *NumberValue) Get128() {
	// TODO: implement Get128
	if nv.Number.Endian == EndianBig {
		if IsLittleEndian {

		} else {

		}
	}
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
	case TypeCodeI128, TypeCodeU128:
		b = append(b, nv.value[0:16]...)
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
	case TypeCodeI128, TypeCodeU128:
		// TODO: add 128bit value support
		if nv.Number.Endian == EndianBig {
			if IsLittleEndian {

			} else {

			}
		} else {

		}
	}
}

func (nv *NumberValue) Set128(value [16]byte) {
	// TODO: implement
}

// Type variant
type Type struct {
	Parent    *Type
	Name      string
	StartLine Line
	EndLine   Line
	Comments  []*Comment
	Optional  *Optional
	Pointer   *Pointer
	Const     *Const
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

func (t *Type) ParentField() (*Type, *Field) {
	if t.Field != nil {
		return t, t.Field
	}
	for p := t.Parent; p != nil; p = p.Parent {
		if p.Field != nil {
			return p, p.Field
		}
	}
	return nil, nil
}

func (t *Type) ParentStruct() (*Type, *Struct) {
	for p := t.Parent; p != nil; p = p.Parent {
		if p.Struct != nil {
			return p, p.Struct
		}
	}
	return nil, nil
}

func (t *Type) ParentConst() (*Type, *Const) {
	for p := t.Parent; p != nil; p = p.Parent {
		if p.Const != nil {
			return p, p.Const
		}
	}
	return nil, nil
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

type Optional struct {
	Type      *Type
	IndexType *Type
}

type Const struct {
	Type  *Type
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
}

func NewStruct(name string, line Line, comments []*Comment) *Struct {
	s := &Struct{Type: &Type{StartLine: line}}
	s.Type.Struct = s
	s.Type.Code = TypeCodeStruct
	s.Type.Name = name
	s.Type.Comments = comments
	return s
}

func (s *Struct) IsUnion() bool {
	return s.Type.Code == TypeCodeUnion
}

func (f *Field) AsUnion() *Struct {
	s := &Struct{Type: &Type{}}
	s.Type.Struct = s
	s.Type.Code = TypeCodeUnion
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

type FieldValue struct {
	Field *Field
	Value Value
}

type Variant struct {
	Type     *Type
	Inline   *Field
	Options  []VariantOption
	Optional bool
}

type VariantOption struct {
	Variant *Variant
	Type    *Type
}

func (vo *VariantOption) Size() int {
	return vo.Type.Size()
}

type Enum struct {
	Type      *Type
	ValueType *Type
	Options   []*EnumOption
}

type EnumOption struct {
	Enum     *Enum
	JsonName string
	Value    NumberValue
}

type Map struct {
	Type  *Type
	Kind  TypeCode
	Key   *Type
	Value *Type
}

type Vector struct {
	Type    *Type
	Kind    TypeCode
	Element *Type
	Length  int
}

type String struct {
	Type        *Type
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
	Constants []*Const
	Structs   []*Struct
	Enums     []*Enum
	Variants  []*Variant
	Types     map[string]*Type
}
