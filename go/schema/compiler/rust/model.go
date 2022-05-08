package rust

import "github.com/moontrade/wap/go/schema/parser"

type Module struct {
	Name     string
	Parent   *Module
	Children []*Module
}

type Struct struct {
	*parser.Struct
	Name string
}

type Union struct {
	*parser.Struct
	Name string
}

type Field struct {
	*parser.Struct
	*parser.Field
	Name string
}

type Enum struct {
	*parser.Enum
}

type EnumOption struct {
	*parser.EnumOption
	Name string
}

type Variant struct {
	*parser.Variant
}

type VariantOption struct {
	*parser.VariantOption
}
