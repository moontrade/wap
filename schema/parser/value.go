package parser

type Value struct {
	Type    *Type
	Const   *Const
	Enum    *EnumOption
	Variant *VariantOption
	Struct  *StructValue
	Number  *NumberValue
	Object  *Object
	Unknown *UnknownValue
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

type UnknownValue struct {
	Value string
}

type Object struct {
	Props  []*ObjectProperty
	ByName map[string]*ObjectProperty
	Curly  bool
}

type ObjectProperty struct {
	Index   int
	Name    string
	Value   interface{}
	HasName bool
}

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

type StructValue struct {
	Fields []FieldValue
	Data   []byte
}

func (sv *StructValue) Append(b []byte) []byte {
	return b
}
