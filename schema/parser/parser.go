package parser

import (
	"fmt"
	"io"
	"math"
	"strconv"
	"strings"
)

type Parser struct {
	r         strings.Reader
	comments  []*Comment
	content   string
	ch        rune
	line      Line
	index     int
	byteIndex int
	col       int
	err       error
	file      *File
}

func NewParser(data []byte) *Parser {
	s := string(data)
	return &Parser{content: s, r: *strings.NewReader(s), file: &File{
		ConstantsByName: make(map[string]*Const),
		Types:           make(map[string]*Type),
	}}
}

type Error struct {
	Line   Line
	Index  int
	Reason string
}

func (e Error) Error() string {
	return fmt.Sprintf("[%d:%d] %s", e.Line.Number, e.Index, e.Reason)
}

type Line struct {
	Data      string
	Number    int
	ColStart  int
	ColEnd    int
	Start     int
	End       int
	ByteStart int
	ByteEnd   int
	Pos       Token
	Done      bool
}

func (l *Line) tryInlineComment() *Comment {
	if l.Pos.Data != "//" {
		return nil
	}
	c := &Comment{
		Inline: true,
		Lines: []CommentLine{{
			Line: *l,
			Text: strings.TrimSpace(l.Data[l.Pos.End+1:]),
		}},
	}
	l.Pos.Data = ""
	l.Pos.Begin = l.End
	l.Pos.End = l.End
	return c
}

// init the
func (l *Line) init() {
loop:
	for start := 0; start < len(l.Data); start++ {
		switch l.Data[start] {
		case '\t', '\r', ' ':
			continue
		default:
			if start > 0 {
				l.ColStart = start
				l.Start += start
				l.ByteStart += start
				l.Data = l.Data[start:]
			}
			break loop
		}
	}
loopend:
	for end := len(l.Data) - 1; end > -1; end-- {
		switch l.Data[end] {
		case '\t', '\r', ' ':
			continue
		default:
			if end < len(l.Data)-1 {
				l.ColEnd = end
				l.End -= len(l.Data) - end - 1
				l.ByteEnd -= len(l.Data) - end - 1
				l.Data = l.Data[0:end]
			}
			break loopend
		}
	}

	l.next()
}

// Next reads the next set of contiguous non-whitespace characters.
func (l *Line) next() bool {
	if l.Done {
		return false
	}
	if len(l.Pos.Data) == 0 {
		if len(l.Data) == 0 {
			l.Done = true
			return false
		}
	}

	start := l.Pos.End
loop:
	for ; start < len(l.Data); start++ {
		c := l.Data[start]
		switch c {
		case ' ', '\t', '\r':
		default:
			break loop
		}
	}
	if start >= len(l.Data) {
		l.Done = true
		return false
	}
	end := start
loop2:
	for ; end < len(l.Data); end++ {
		c := l.Data[end]
		switch c {
		case '/':
			if end == start {
				end++

				if end <= len(l.Data)-1 {
					switch l.Data[end] {
					case '/', '*':
						end++
					}
				}
			}
			break loop2

		case ':':
			if end == start {
				end++
				if end <= len(l.Data)-1 {
					switch l.Data[end] {
					case ':':
						end++
					}
				}
			}
			break loop2

		case '.':
			if end == start {
				end++
				if end <= len(l.Data)-1 {
					switch l.Data[end] {
					case '.':
						end++
						if end <= len(l.Data)-1 {
							switch l.Data[end] {
							case '.':
								end++
							}
						}
					}
				}
			}
			break loop2

		case '|':
			if end == start {
				end++
				if end <= len(l.Data)-1 {
					switch l.Data[end] {
					case '|':
						end++
					}
				}
			}
			break loop2

		case '{', '}', '@', '#', '"', '=', '<', '>', '[', ']', ';', ',', '^', '&', '*':
			if end == start {
				end++
			}
			break loop2
		case ' ', '\t', '\r':
			break loop2
		default:
		}
	}

	l.Pos.Begin = start
	l.Pos.End = end
	l.Pos.Data = l.Data[start:end]

	return true
}

type Token struct {
	Begin int
	End   int
	Data  string
	//ColStart     int
	//ColEnd       int
	//ColByteStart int
	//ColByteEnd   int
	//Err error
}

func (p *Parser) inlineComment(t *Type) bool {
	c := p.line.tryInlineComment()
	if c == nil {
		return p.next()
	}
	t.Comments = append(t.Comments, c)
	return p.nextLine()
}

func (p *Parser) advance() bool {
	var size int
	p.ch, size, p.err = p.r.ReadRune()
	p.index++
	if p.err != nil {
		if p.err == io.EOF {
			p.err = io.ErrUnexpectedEOF
		}
		return false
	}
	p.byteIndex += size
	return true
}

func (p *Parser) nextLine() bool {
startOver:
	next := Line{
		Number:    p.line.Number + 1,
		Start:     p.index,
		ByteStart: p.byteIndex,
	}
	for {
		if !p.advance() {
			return false
		}

		if p.ch == '\n' {
			next.Data = p.content[next.ByteStart : p.byteIndex-1]
			next.End = p.index - 1
			next.ByteEnd = p.byteIndex - 1
			next.init()
			p.line = next

			// Skip empty lines
			if len(p.line.Data) == 0 {
				goto startOver
			}

			// Parse comments automatically
			switch p.word() {
			case "//":
				var comment *Comment
				if len(p.comments) > 0 {
					comment = p.comments[len(p.comments)-1]
					if comment.Star {
						comment = &Comment{}
						p.comments = append(p.comments, comment)
					}
				} else {
					comment = &Comment{}
					p.comments = append(p.comments, comment)
				}
				if !comment.Star && len(comment.Lines) > 0 {
					if comment.Lines[len(comment.Lines)-1].Line.Number != next.Number-1 {
						comment = &Comment{
							Star:  false,
							Lines: nil,
						}
						p.comments = append(p.comments, comment)
					}
				}
				comment.Lines = append(comment.Lines, CommentLine{
					Line: p.line,
					Text: strings.TrimSpace(next.Data[2:]),
				})
				goto startOver

			case "/*":
				comment := &Comment{Star: true}
				p.comments = append(p.comments, comment)
				index := strings.Index(next.Data, "*/")
				if index > -1 {
					comment.Lines = append(comment.Lines, CommentLine{Line: next, Text: strings.TrimSpace(next.Data[2:index])})
					if strings.TrimSpace(next.Data[index+2:]) != "" {
						p.err = p.error("multi-line comments /* */ cannot have any text after the closing '*/'")
						return false
					}
					goto startOver
				}
				comment.Lines = append(comment.Lines, CommentLine{
					Line: p.line,
					Text: strings.TrimSpace(next.Data[2:]),
				})

				next = Line{
					Number:    p.line.Number + 1,
					Start:     p.index,
					ByteStart: p.byteIndex,
				}

			loop2:
				for {
					if !p.advance() {
						return false
					}

					switch p.ch {
					case '\n':
						next.Data = p.content[next.ByteStart : p.byteIndex-1]
						next.End = p.index - 1
						next.ByteEnd = p.byteIndex - 1
						p.line = next

						comment.Lines = append(comment.Lines, CommentLine{Line: next, Text: next.Data})

						next = Line{
							Number:    p.line.Number + 1,
							Start:     p.index,
							ByteStart: p.byteIndex,
						}
					case '*':
						if !p.advance() {
							return false
						}
						if p.ch == '/' {
							next.Data = p.content[next.ByteStart : p.byteIndex-2]
							next.End = p.index - 2
							next.ByteEnd = p.byteIndex - 2
							p.line = next

							if strings.TrimSpace(next.Data) != "" {
								p.err = p.error("multi-line comments /* */ cannot have any text after the closing '*/'")
								return false
							}
							break loop2
						}
					}

				}
				goto startOver
			}

			break
		}
	}

	return true
}

func (p *Parser) nextWord() bool {
	return p.line.next()
}

func (p *Parser) next() bool {
	if !p.nextWord() {
		if !p.nextLine() {
			return false
		}
	}
	return true
}

func (p *Parser) word() string {
	return p.line.Pos.Data
}

func (p *Parser) isWord(v string) bool {
	return p.line.Pos.Data == v
}

func (p *Parser) error(format string, args ...interface{}) error {
	p.err = &Error{
		Line:   p.line,
		Index:  p.line.Pos.End,
		Reason: fmt.Sprintf(format, args...),
	}
	return p.err
}

func (p *Parser) NextLine() (Line, error) {
	if !p.nextLine() {
		return Line{}, p.err
	}
	return p.line, nil
}

func (p *Parser) flushComments() []*Comment {
	c := p.comments
	p.comments = nil
	return c
}

func (p *Parser) Parse() error {
loop:
	for {
		if p.err != nil {
			return p.err
		}
		if !p.nextLine() {
			break loop
		}

	run:

		switch p.word() {
		case "namespace":
			fmt.Printf("namespace = %s\n", p.combineRemainingWords())

		case "packed":
			if !p.nextWord() {
				return p.error("expected 'struct' after 'packed' keyword")
			}

		case "type", "using", "use", "alias":
			keyword := p.word()
			p.nextWord()
			fmt.Printf("%s: %s\n", keyword, p.word())

		case "const":
			p.nextWord()
			fmt.Println("const:", p.word())

		case "struct":
			l := p.line
			if !p.nextWord() {
				return p.error("expected name after 'struct' keyword")
			}
			fmt.Println("struct:", p.word())
			name := p.word()
			if !p.validateStructName(name) {
				return p.err
			}

			s := NewStruct(name, l, p.flushComments())
			p.file.Structs = append(p.file.Structs, s)
			if !p.expectAfterWhitespace("{") {
				return p.error("expected '{' after struct name '%s'", name)
			}
			if !p.nextWord() {
				if !p.nextLine() {
					return p.err
				}
			}
			p.parseStruct(s)
			if p.err != nil {
				if p.err == io.EOF {
					return p.err
				}
				return p.err
			}
			goto run

		case "protocol":
			if !p.nextWord() {

			}

			fmt.Println("protocol:", p.word())
		}
	}
	return p.err
}

func (p *Parser) parseNestedStruct(parent *Struct) bool {
	defer func() {
		if p.err == io.EOF {
			p.err = io.ErrUnexpectedEOF
		}
	}()
	l := p.line
	if !p.nextWord() {
		_ = p.error("expected name after 'struct' keyword")
		return false
	}
	fmt.Println("struct:", p.word())
	name := p.word()
	if !p.validateStructName(name) {
		return false
	}

	s := NewStruct(name, l, p.flushComments())
	s.Parent = parent
	parent.Inner = append(parent.Inner, s.Type)
	p.file.Structs = append(p.file.Structs, s)
	if !p.expectAfterWhitespace("{") {
		_ = p.error("expected '{' after struct name '%s'", name)
		return false
	}
	if !p.nextWord() {
		if !p.nextLine() {
			return false
		}
	}
	p.parseStruct(s)
	if p.err != nil {
		if p.err == io.EOF {
			p.err = io.ErrUnexpectedEOF
		}
		return false
	}
	return true
}

const maxPadSize = 1024 * 1024 * 64

func (p *Parser) parseStruct(s *Struct) {
	if p.err != nil {
		return
	}

loop:
	for !p.isWord("}") {
		if p.word() != "@" {
			// maybe nested type?
			switch p.word() {
			case "..", "...":
				if !p.nextWord() {
					_ = p.error("padding expects an integer value: ... 4")
					return
				}

				size, err := strconv.ParseInt(p.word(), 10, 32)
				if err != nil {
					_ = p.error("padding expects an integer value: ... 4")
					return
				}
				if size < 1 || size > maxPadSize {
					_ = p.error("padding value is out of bounds (0 to %d): %d", maxPadSize, size)
					return
				}
				field := s.NewField(p.line, p.flushComments())
				field.Type.Code = TypeCodePad
				field.Size = int(size)
				if !p.next() {
					return
				}
				goto loop

			case "struct":
				if !p.parseNestedStruct(s) {
					return
				}
				goto loop
			case "enum":
				goto loop
			case "variant":
				goto loop
			case "using":
				goto loop
			}
		}

		field := s.NewField(p.line, p.flushComments())

		if p.word() != "@" {
			// Try parsing tag number without '@' prefix
			number, err := strconv.ParseInt(p.word(), 10, 32)
			if err == nil {
				if number < 1 {
					_ = p.error("field number must be greater than 0")
					return
				}
				field.Number = int(number)

				if !p.next() {
					return
				}
				// consume optional colon
				if p.word() == ":" {
					if !p.next() {
						return
					}
				}
			}
		} else {
			if !p.parseTagNumber(field) {
				return
			}
			if !p.nextWord() {
				_ = p.error("expected field name")
				return
			}
		}

		// union is declared inline just like C/C++
		if p.word() == "union" {
			if !p.expectAfterWhitespace("{") {
				return
			}

			// union is a variation of a struct so let's just recurse
			p.parseStruct(field.AsUnion())
			if p.err != nil {
				return
			}

			if !p.next() {
				return
			}

			// Is this the structure's closing curly brace?
			if p.word() == "}" {
				return
			}
		}

		// Validate and set field name
		if !isValidName(p.word()) {
			_ = p.error("invalid field name '%s'", p.word())
			return
		}
		field.Name = p.word()
		if !p.next() {
			return
		}

		// Expecting either || or a type declaration
		for p.word() == "||" {
			if !p.next() {
				return
			}

			altType := p.word()
			if !p.next() {
				return
			}
			if p.word() != ":" {
				p.err = p.error("expected colon after || alternative name declaration: (e.g. j:name)")
				return
			}

			switch strings.ToLower(altType) {
			case "j", "json":
				if !p.next() {
					return
				}
				if p.word() == "\"" {
					name := ""
					for {
						if !p.next() {
							return
						}
						if p.word() == "\"" {
							break
						}
						name += p.word()
					}

					field.JsonNames = append(field.JsonNames, name)
				} else {
					field.JsonNames = append(field.JsonNames, p.word())
				}
				if !p.next() {
					return
				}

			default:
				p.err = p.error("unknown alt name type: %s", altType)
				return
			}
		}

		if !p.parseType(field.Type) {
			return
		}
	}

	s.Type.EndLine = p.line

	if !p.next() {
		if p.err == io.ErrUnexpectedEOF {
			p.err = io.EOF
		}
	}
}

func (p *Parser) parseEnum(t *Type) {

}

func (p *Parser) parseBits(t *Type) {}

func (p *Parser) parseVariant(e *Type) {

}

const maxInlineStringSize = 1024 * 1024

func (p *Parser) parseType(t *Type) bool {
	if p.err != nil {
		return false
	}

	switch p.word() {
	case "bool", "boolean":
		t.Name = Bool.Name
		t.Code = Bool.Code
		t.Number = Bool.Number
	case "i8", "I8", "int8", "Int8", "schar", "sbyte", "int8_t":
		t.Name = I8.Name
		t.Code = I8.Code
		t.Number = I8.Number
	case "u8", "U8", "uint8", "UInt8", "byte", "char", "uchar", "uint8_t":
		t.Name = U8.Name
		t.Code = U8.Code
		t.Number = U8.Number

	case "i16", "I16", "int16", "Int16", "short", "int16_t":
		t.Name = I16.Name
		t.Code = I16.Code
		t.Number = I16.Number
	case "i16b", "I16B", "i16be", "i16BE", "I16BE", "int16be", "int16BE", "Int16BE":
		t.Name = I16B.Name
		t.Code = I16B.Code
		t.Number = I16B.Number
	case "i16n", "i16ne", "i16N", "i16NE", "int16ne", "int16NE", "Int16NE":
		t.Name = I16N.Name
		t.Code = I16N.Code
		t.Number = I16N.Number

	case "u16", "u16l", "u16le", "uint16", "uint16l", "uint16le", "UInt16", "Uint16", "ushort", "uint16_t":
		t.Name = U16.Name
		t.Code = U16.Code
		t.Number = U16.Number
	case "u16b", "uint16b", "uint16be", "UInt16BE", "Uint16BE":
		t.Name = U16B.Name
		t.Code = U16B.Code
		t.Number = U16B.Number
	case "u16n", "uint16n", "uint16ne", "UInt16NE", "Uint16NE":
		t.Name = U16N.Name
		t.Code = U16N.Code
		t.Number = U16N.Number

	case "i32", "I32", "int32", "Int32", "int", "int32_t":
		t.Name = I32.Name
		t.Code = I32.Code
		t.Number = I32.Number
	case "i32b", "i32be", "i32BE", "int32be", "int32BE", "Int32BE":
		t.Name = I32B.Name
		t.Code = I32B.Code
		t.Number = I32B.Number
	case "i32n", "i32ne", "i32N", "i32NE", "int32ne", "int32NE", "Int32NE":
		t.Name = I32N.Name
		t.Code = I32N.Code
		t.Number = I32N.Number

	case "u32", "uint32", "UInt32", "Uint32", "uint", "uint32_t":
		t.Name = U32.Name
		t.Code = U32.Code
		t.Number = U32.Number
	case "u32b", "u32be", "u32BE", "unt32be", "unt32BE", "Unt32BE":
		t.Name = U32B.Name
		t.Code = U32B.Code
		t.Number = U32B.Number
	case "u32n", "u32ne", "u32N", "u32NE", "U32N", "U32NE", "uint32ne", "uint32NE", "UInt32NE":
		t.Name = U32N.Name
		t.Code = U32N.Code
		t.Number = U32N.Number

	case "i64", "I64", "int64", "Int64", "long", "int64_t":
		t.Name = I64.Name
		t.Code = I64.Code
		t.Number = I64.Number
	case "i64b", "I64B", "i64be", "i64BE", "I64BE", "int64be", "int64BE", "Int64BE":
		t.Name = I64B.Name
		t.Code = I64B.Code
		t.Number = I64B.Number
	case "i64n", "I64N", "i64ne", "i64N", "i64NE", "I64NE", "int64ne", "int64NE", "Int64NE":
		t.Name = I64N.Name
		t.Code = I64N.Code
		t.Number = I64N.Number

	case "u64", "U64", "uint64", "UInt64", "Uint64", "ulong", "uint64_t":
		t.Name = U64.Name
		t.Code = U64.Code
		t.Number = U64.Number
	case "u64b", "U64B", "u64be", "u64BE", "U64BE", "unt64be", "unt64BE", "Unt64BE":
		t.Name = U64B.Name
		t.Code = U64B.Code
		t.Number = U64B.Number
	case "u64n", "U64N", "u64ne", "u64N", "u64NE", "U64NE", "uint64ne", "uint64NE", "UInt64NE":
		t.Name = U64N.Name
		t.Code = U64N.Code
		t.Number = U64N.Number

	case "f32", "F32", "float32", "Float32", "float", "float32_t":
		t.Name = F32.Name
		t.Code = F32.Code
		t.Number = F32.Number
	case "f32b", "F32B", "f32be", "f32BE", "F32BE", "float32be", "float32BE", "Float32BE":
		t.Name = F32B.Name
		t.Code = F32B.Code
		t.Number = F32B.Number
	case "f32n", "F32N", "f32ne", "f32N", "f32NE", "F32NE", "float32ne", "float32NE", "Float32NE":
		t.Name = F32N.Name
		t.Code = F32N.Code
		t.Number = F32N.Number

	case "f64", "F64", "float64", "Float64", "double", "float64_t":
		t.Name = F64.Name
		t.Code = F64.Code
		t.Number = F64.Number
	case "f64b", "F64B", "f64be", "f64BE", "F64BE", "float64be", "float64BE", "Float64BE":
		t.Name = F64B.Name
		t.Code = F64B.Code
		t.Number = F64B.Number
	case "f64n", "F64N", "f64ne", "f64N", "f64NE", "F64NE", "float64ne", "float64NE", "Float64NE":
		t.Name = F64N.Name
		t.Code = F64N.Code
		t.Number = F64N.Number

	case "string":

	case "map":
	case "ordered_map":
	case "tree_map":
	case "set":
	case "ordered_set":
	case "tree_set":
	case "[":
		if !p.nextWord() {
			_ = p.error("incomplete array/vector type declaration")
			return false
		}

		vector := &Vector{Element: &Type{}}
		t.Vector = vector

		// Is it a vector?
		if p.word() == "]" {
			if !p.nextWord() {
				_ = p.error("incomplete vector type declaration")
				return false
			}
			t.Code = TypeCodeVector
			t.Vector.Kind = TypeCodeVector
		}

		length, err := strconv.ParseInt(p.word(), 10, 32)
		if err != nil {
			_ = p.error("array size must be a positive integer: %s", p.word())
			return false
		}
		vector.Length = int(length)
		t.Code = TypeCodeArray
		t.Vector.Kind = TypeCodeArray

		if !p.nextWord() {
			_ = p.error("incomplete vector type declaration")
			return false
		}

	case "variant":
	case "enum":
	case "struct":

	case "union":
		p.err = p.error("unions may only be declared inline")
		return false

	default:
		// maybe a string variant?
		if strings.HasPrefix(p.word(), "string") {
			length, err := strconv.ParseInt(p.word()[len("string"):], 10, 32)
			if err != nil {
				_ = p.error("string size must be an integer: %s", p.word()[len("string"):])
				return false
			}
			if length > maxInlineStringSize {
				_ = p.error("string size must be an integer: %s", p.word()[len("string"):])
				return false
			}
		}

		// user defined type
	}

	if !p.nextWord() {
		if !p.nextLine() {
			return false
		}
	} else if p.word() == "//" {
		return p.inlineComment(t)
	}

	// consume optional semicolon or comma
	if p.word() == ";" || p.word() == "," {
		if !p.nextWord() {
			return p.nextLine()
		}
		if p.word() == "//" {
			return p.inlineComment(t)
		}
		return true
	}

	if p.word() == "=" {
		if !p.next() {
			return false
		}
		if !p.parseValue(t) {
			return false
		}
		if !p.nextWord() {
			return p.nextLine()
		}

		// consume optional semicolon or comma
		if p.word() == ";" || p.word() == "," {
			if !p.nextWord() {
				return p.nextLine()
			}
			if p.word() == "//" {
				return p.inlineComment(t)
			}
			return true
		}

		if p.word() == "//" {
			return p.inlineComment(t)
		}

		p.err = p.error("unexpected text after value declaration: %s", p.word())
		return false
	}

	return true
}

func (p *Parser) parseValue(t *Type) bool {
	switch t.Code {
	case TypeCodeBool:
		switch strings.ToLower(p.word()) {
		case "0", "f", "false", "n", "no":
			t.Value = &Value{Number: t.Number.ToValue(0)}
		case "1", "t", "true", "y", "yes":
			t.Value = &Value{Number: t.Number.ToValue(1)}
		default:
			p.err = p.error("invalid bool value: %s", p.word())
			return false
		}
		return true

	case TypeCodeI8:
		switch strings.ToLower(p.word()) {
		case "min":
			t.Value = &Value{Number: t.Number.ToValueSigned(math.MinInt8)}
		case "max":
			t.Value = &Value{Number: t.Number.ToValueSigned(math.MaxInt8)}
		default:
			number, err := strconv.ParseInt(p.word(), 10, 64)
			if err != nil {
				p.err = p.error("invalid i8 value: %s", p.word())
				return false
			}
			if number < math.MinInt8 || number > math.MaxInt8 {
				p.err = p.error("i8 value is out of range [%d to %d]: %s", math.MinInt8, math.MaxInt8, p.word())
				return false
			}
			t.Value = &Value{Number: t.Number.ToValueSigned(number)}
			return true
		}
	case TypeCodeI16:
		switch strings.ToLower(p.word()) {
		case "min":
			t.Value = &Value{Number: t.Number.ToValueSigned(math.MinInt16)}
		case "max":
			t.Value = &Value{Number: t.Number.ToValueSigned(math.MaxInt16)}
		default:
			number, err := strconv.ParseInt(p.word(), 10, 64)
			if err != nil {
				p.err = p.error("invalid i16 value: %s", p.word())
				return false
			}
			if number < math.MinInt16 || number > math.MaxInt16 {
				p.err = p.error("i16 value is out of range [%d to %d]: %s", math.MinInt16, math.MaxInt16, p.word())
				return false
			}
			t.Value = &Value{Number: t.Number.ToValueSigned(number)}
			return true
		}

	case TypeCodeI32:
		switch strings.ToLower(p.word()) {
		case "min":
			t.Value = &Value{Number: t.Number.ToValueSigned(math.MinInt32)}
		case "max":
			t.Value = &Value{Number: t.Number.ToValueSigned(math.MaxInt32)}
		default:
			number, err := strconv.ParseInt(p.word(), 10, 64)
			if err != nil {
				p.err = p.error("invalid i32 value: %s", p.word())
				return false
			}
			if number < math.MinInt32 || number > math.MaxInt32 {
				p.err = p.error("i32 value is out of range [%d to %d]: %s", math.MinInt32, math.MaxInt32, p.word())
				return false
			}
			t.Value = &Value{Number: t.Number.ToValueSigned(number)}
			return true
		}

	case TypeCodeI64:
		switch strings.ToLower(p.word()) {
		case "min":
			t.Value = &Value{Number: t.Number.ToValueSigned(math.MinInt64)}
		case "max":
			t.Value = &Value{Number: t.Number.ToValueSigned(math.MaxInt64)}
		default:
			number, err := strconv.ParseInt(p.word(), 10, 64)
			if err != nil {
				p.err = p.error("invalid i64 value: %s", p.word())
				return false
			}
			t.Value = &Value{Number: t.Number.ToValueSigned(number)}
			return true
		}

	case TypeCodeU8:
		switch strings.ToLower(p.word()) {
		case "min":
			t.Value = &Value{Number: t.Number.ToValue(0)}
		case "max":
			t.Value = &Value{Number: t.Number.ToValue(math.MaxUint8)}
		default:
			number, err := strconv.ParseInt(p.word(), 10, 64)
			if err != nil {
				p.err = p.error("invalid u8 value: %s", p.word())
				return false
			}
			if number < 0 || number > math.MaxUint8 {
				p.err = p.error("u8 value is out of range [%d to %d]: %s", 0, math.MaxUint8, p.word())
				return false
			}
			return true
		}

	case TypeCodeU16:
		switch strings.ToLower(p.word()) {
		case "min":
			t.Value = &Value{Number: t.Number.ToValue(0)}
		case "max":
			t.Value = &Value{Number: t.Number.ToValue(math.MaxUint16)}
		default:
			number, err := strconv.ParseInt(p.word(), 10, 64)
			if err != nil {
				p.err = p.error("invalid u16 value: %s", p.word())
				return false
			}
			if number < 0 || number > math.MaxUint16 {
				p.err = p.error("u16 value is out of range [%d to %d]: %s", 0, math.MaxUint16, p.word())
				return false
			}
			t.Value = &Value{Number: t.Number.ToValue(uint64(number))}
			return true
		}

	case TypeCodeU32:
		switch strings.ToLower(p.word()) {
		case "min":
			t.Value = &Value{Number: t.Number.ToValue(0)}
		case "max":
			t.Value = &Value{Number: t.Number.ToValue(math.MaxUint32)}
		default:
			number, err := strconv.ParseInt(p.word(), 10, 64)
			if err != nil {
				p.err = p.error("invalid u32 value: %s", p.word())
				return false
			}
			if number < 0 || number > math.MaxUint32 {
				p.err = p.error("u32 value is out of range [%d to %d]: %s", 0, math.MaxUint32, p.word())
				return false
			}
			t.Value = &Value{Number: t.Number.ToValue(uint64(number))}
			return true
		}

	case TypeCodeU64:
		switch strings.ToLower(p.word()) {
		case "min":
			t.Value = &Value{Number: t.Number.ToValue(0)}
		case "max":
			t.Value = &Value{Number: t.Number.ToValue(math.MaxUint64)}
		default:
			number, err := strconv.ParseUint(p.word(), 10, 64)
			if err != nil {
				p.err = p.error("invalid u64 value: %s", p.word())
				return false
			}
			t.Value = &Value{Number: t.Number.ToValue(number)}
			return true
		}

	case TypeCodeF32:
		switch strings.ToLower(p.word()) {
		case "min":
			t.Value = &Value{Number: t.Number.ToValue(0)}
		case "max":
			t.Value = &Value{Number: t.Number.ToValue(math.Float64bits(math.MaxFloat32))}
		default:
			number, err := strconv.ParseFloat(p.word(), 64)
			if err != nil {
				p.err = p.error("invalid f64 value: %s", p.word())
				return false
			}
			if number > math.MaxFloat32 {
				p.err = p.error("f32 value is out of range: %s", p.word())
				return false
			}
			t.Value = &Value{Number: t.Number.ToValue(math.Float64bits(number))}
			return true
		}

	case TypeCodeF64:
		switch strings.ToLower(p.word()) {
		case "min":
			t.Value = &Value{Number: t.Number.ToValue(0)}
		case "max":
			t.Value = &Value{Number: t.Number.ToValue(math.Float64bits(math.MaxFloat64))}
		default:
			number, err := strconv.ParseFloat(p.word(), 64)
			if err != nil {
				p.err = p.error("invalid f64 value: %s", p.word())
				return false
			}
			t.Value = &Value{Number: t.Number.ToValue(math.Float64bits(number))}
			return true
		}

	case TypeCodeEnum:

	case TypeCodeStruct:

	case TypeCodeUnion:

	case TypeCodeVariant:

	case TypeUnknown:
		// treat as any
	}
	return true
}

func (p *Parser) parseTagNumber(f *Field) bool {
	if !p.next() {
		return false
	}
	number, err := strconv.ParseInt(p.word(), 10, 32)
	if err != nil || number < 1 {
		_ = p.error("expected an integer greater than 0 after '@'")
		return false
	}
	f.Number = int(number)
	return true
}

func (p *Parser) combineRemainingWords() string {
	if !p.nextWord() {
		return ""
	}
	r := p.word()

	for p.nextWord() {
		if p.isWord("//") {
			p.comments = append(p.comments, &Comment{
				Lines: []CommentLine{{
					Line: p.line,
					Text: strings.TrimSpace(p.line.Data[p.line.Pos.Begin+2:]),
				}},
			})
			p.nextLine()
			return r
		}
		if p.isWord("/*") {
			p.parseSlashStarComment()
			return r
		}

		r += p.word()
	}

	return r
}

func (p *Parser) expectAfterWhitespace(v string) bool {
	if p.err != nil {
		return false
	}
	if p.word() == v {
		return true
	}
	for {
		if !p.nextWord() {
			if !p.nextLine() {
				return false
			}
			continue
		}
		if p.word() == "" {
			continue
		}
		return p.word() == v
	}
}

func (p *Parser) parseDoubleSlashComment() {
	comment := &Comment{}
	comment.Lines = append(comment.Lines, CommentLine{
		Line: p.line,
		Text: strings.TrimSpace(p.line.Data[p.line.Pos.Begin+2:]),
	})
	p.comments = append(p.comments, comment)
	for {
		if !p.nextLine() || !p.isWord("//") {
			return
		}
		comment.Lines = append(comment.Lines, CommentLine{
			Line: p.line,
			Text: strings.TrimSpace(p.line.Data[p.line.Pos.Begin+2:]),
		})
	}
}

func (p *Parser) validateStructName(name string) bool {
	if !isValidName(name) {
		_ = p.error("struct name '%s' is not valid", name)
		return false
	}
	return true
}

func isValidName(v string) bool {
	if len(v) == 0 {
		return false
	}

	for i, ch := range v {
		if i == 0 {
			if !isValidNameFirstChar(ch) {
				return false
			}
		} else {
			if !isValidNameAfterFirstChar(ch) {
				return false
			}
		}
	}
	return true
}

func hasValidNameFirstChar(v string) bool {
	for _, ch := range v {
		return isValidNameFirstChar(ch)
	}
	return false
}

func isValidNameFirstChar(ch rune) bool {
	switch {
	case ch >= 'a' && ch <= 'z':
		return true
	case ch >= 'A' && ch < 'Z':
		return true
	case ch == '_' || ch == '$':
		return true
	}
	return false
}

func isValidNameAfterFirstChar(ch rune) bool {
	switch {
	case ch >= 'a' && ch <= 'z':
		return true
	case ch >= 'A' && ch < 'Z':
		return true
	case ch == '_' || ch == '$':
		return true
	case ch >= '0' && ch <= '9':
		return true
	}
	return false
}

func (p *Parser) parseSlashStarComment() {
	comment := &Comment{}
	comment.Lines = append(comment.Lines, CommentLine{
		Line: p.line,
		Text: strings.TrimSpace(p.line.Data[p.line.Pos.Begin+2:]),
	})
	p.comments = append(p.comments, comment)
	for {
		if !p.nextLine() {
			return
		}
		index := strings.Index(p.line.Data, "*/")
		if index > -1 {
			text := p.line.Data[0:index]
			if len(strings.TrimSpace(text)) > 2 {
				p.err = &Error{
					Line:   p.line,
					Index:  index + 2,
					Reason: "unexpected text after closing comment identifier '*/'",
				}
				return
			}
			comment.Lines = append(comment.Lines, CommentLine{
				Line: p.line,
				Text: text,
			})
			return
		} else {
			comment.Lines = append(comment.Lines, CommentLine{
				Line: p.line,
				Text: p.line.Data,
			})
		}
	}
}
