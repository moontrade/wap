package parser

import (
	"fmt"
	"io"
	"math"
	"strconv"
	"strings"
	"unsafe"
)

const maxInlineStringSize = 1024 * 1024

type Parser struct {
	r        strings.Reader
	content  string
	ch       rune
	line     Line
	cidx     int
	bidx     int
	col      int
	err      error
	file     *File
	comments []*Comment
}

func NewParser(data []byte) *Parser {
	s := string(data)
	return &Parser{content: s, r: *strings.NewReader(s), file: &File{
		Types: make(map[string]*Type),
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
	ColStart  int // offset of the first non-whitespace rune
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
	// Trim leading whitespace
leading:
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
			break leading
		}
	}
	// Trim trailing whitespace
trailing:
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
			break trailing
		}
	}

	if len(l.Data) == 0 {
		return
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

		case '*':
			if end == start {
				end++
				if end <= len(l.Data)-1 {
					switch l.Data[end] {
					case '/':
						end++
					}
				}
			}
			break loop2

		case '{', '}', '@', '#', '"', '=', '<', '>', '[', ']', ';', ',', '^', '&':
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
	p.cidx++
	if p.err != nil {
		if p.err == io.EOF {
			p.err = io.ErrUnexpectedEOF
		}
		return false
	}
	p.bidx += size
	return true
}

func (p *Parser) nextLine() bool {
startOver:
	next := Line{
		Number:    p.line.Number + 1,
		Start:     p.cidx,
		ByteStart: p.bidx,
	}
	for {
		if !p.advance() {
			return false
		}
		if p.ch != '\n' {
			continue
		}

		next.Data = p.content[next.ByteStart : p.bidx-1]
		next.End = p.cidx - 1
		next.ByteEnd = p.bidx - 1
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
			// Previous comment?
			if len(p.comments) > 0 {
				comment = p.comments[len(p.comments)-1]
				// If start then start a new group.
				if comment.Star {
					comment = &Comment{}
					p.comments = append(p.comments, comment)
				}
			} else {
				// Start a new group.
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
					return p.error("multi-line comments /* */ cannot have any text after the closing '*/'")
				}
				goto startOver
			}
			comment.Lines = append(comment.Lines, CommentLine{
				Line: p.line,
				Text: strings.TrimSpace(next.Data[2:]),
			})

			next = Line{
				Number:    p.line.Number + 1,
				Start:     p.cidx,
				ByteStart: p.bidx,
			}

		loop2:
			for {
				if !p.advance() {
					return false
				}

				switch p.ch {
				case '\n':
					next.Data = p.content[next.ByteStart : p.bidx-1]
					next.End = p.cidx - 1
					next.ByteEnd = p.bidx - 1
					p.line = next

					comment.Lines = append(comment.Lines, CommentLine{Line: next, Text: next.Data})

					next = Line{
						Number:    p.line.Number + 1,
						Start:     p.cidx,
						ByteStart: p.bidx,
					}
				case '*':
					if !p.advance() {
						return false
					}
					if p.ch == '/' {
						next.Data = p.content[next.ByteStart : p.bidx-2]
						next.End = p.cidx - 2
						next.ByteEnd = p.bidx - 2
						p.line = next

						if strings.TrimSpace(next.Data) != "" {
							return p.error("multi-line comments /* */ cannot have any text after the closing '*/'")
						}
						break loop2
					}
				}

			}
			goto startOver
		}

		break
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

func (p *Parser) error(format string, args ...interface{}) bool {
	l := p.line
	l.Number--
	p.err = &Error{
		Line:   l,
		Index:  l.ColStart + l.Pos.Begin,
		Reason: fmt.Sprintf(format, args...),
	}
	return false
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
	if !p.next() {
		return p.err
	}
	for {
		if p.err != nil {
			return p.err
		}

		switch p.word() {
		case "namespace":
			fmt.Printf("namespace = %s\n", p.combineRemainingWords())
			if !p.nextLine() {
				return p.err
			}

		case "packed":
			if !p.nextWord() {
				p.error("expected 'struct' after 'packed' keyword")
				return p.err
			}
			if !p.nextLine() {
				return p.err
			}

		case "type", "using", "use", "alias":
			keyword := p.word()
			if !p.next() {
				return p.err
			}
			fmt.Printf("%s: %s\n", keyword, p.word())
			if !p.nextLine() {
				return p.err
			}

		case "const":
			if !p.next() {
				return p.err
			}
			fmt.Println("const:", p.word())
			if !p.nextLine() {
				return p.err
			}

		case "struct", "union":
			t := &Type{}
			if !p.parseType(t, false) {
				return p.err
			}

		case "protocol":
			if !p.nextWord() {

			}

			fmt.Println("protocol:", p.word())
			if !p.nextLine() {
				return p.err
			}

		default:
			if !p.nextLine() {
				return p.err
			}
		}
	}
}

//func (p *Parser) parseNestedStruct(parent *Struct) bool {
//	defer func() {
//		if p.err == io.EOF {
//			p.err = io.ErrUnexpectedEOF
//		}
//	}()
//	l := p.line
//	if !p.nextWord() {
//		_ = p.error("expected name after 'struct' keyword")
//		return false
//	}
//	fmt.Println("struct:", p.word())
//	name := p.word()
//	if !p.validateDeclaredName(name, "struct") {
//		return false
//	}
//
//	s := NewStruct(name, l, p.flushComments())
//	s.Parent = parent
//	parent.Inner = append(parent.Inner, s.Type)
//	p.file.Structs = append(p.file.Structs, s)
//	if !p.expectAfterWhitespace("{") {
//		_ = p.error("expected '{' after struct name '%s'", name)
//		return false
//	}
//	if !p.nextWord() {
//		if !p.nextLine() {
//			return false
//		}
//	}
//	p.parseStruct(s)
//	if p.err != nil {
//		if p.err == io.EOF {
//			p.err = io.ErrUnexpectedEOF
//		}
//		return false
//	}
//	return true
//}

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
			// padding?
			case "..", "...":
				if !p.nextWord() {
					p.error("padding expects an integer value: ... 4")
					return
				}

				size, err := strconv.ParseInt(p.word(), 10, 32)
				if err != nil {
					_ = p.error("padding expects an integer value: ... 4")
					return
				}
				if size < 1 || size > maxPadSize {
					p.error("padding value is out of bounds (0 to %d): %d", maxPadSize, size)
					return
				}
				field := s.NewField(p.line, p.flushComments())
				field.Type.Code = TypeCodePad
				field.Size = int(size)

				if !p.nextWord() {
					if !p.nextLine() {
						return
					}
					continue loop
				} else if p.word() == "//" {
					if !p.inlineComment(field.Type) {
						return
					}
				}

				// consume optional semicolon or comma
				if p.word() == ";" || p.word() == "," {
					if !p.nextWord() {
						if !p.nextLine() {
							return
						}
						continue loop
					}
					if p.word() == "//" {
						if !p.inlineComment(field.Type) {
							return
						}
					}
					continue loop
				}
				continue loop

			case "struct", "enum", "variant":
				t := &Type{Parent: s.Type}
				if !p.parseType(t, true) {
					return
				}
				continue loop
			case "using":
				continue loop
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
				p.error("expected colon after || alternative name declaration: (e.g. j:name)")
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

			case "p", "pb":
				if !p.next() {
					return
				}

				number, err := strconv.ParseInt(p.word(), 10, 32)
				if err != nil {
					p.error("invalid protocol buffer field number: %s", p.word())
					return
				}
				if number < 1 || number > math.MaxInt32 {
					p.error("protocol buffer field number out of range (1 to %d): %d", math.MaxInt32, number)
				}
				field.ProtoNumber = int(number)
				if !p.next() {
					return
				}

			default:
				p.error("unknown alt name type: %s", altType)
				return
			}
		}

		// optional colon separator before type declaration.
		if p.word() == ":" {
			if !p.next() {
				return
			}
		}

		if !p.parseType(field.Type, true) {
			return
		}
	}

	s.Type.EndLine = p.line

	if _, parent := s.Type.ParentStruct(); parent != nil {
		parent.Inner = append(parent.Inner, s.Type)
		if !p.next() {
			return
		}
	} else {
		p.file.Structs = append(p.file.Structs, s)
		if !p.next() {
			if p.err == io.ErrUnexpectedEOF {
				p.err = io.EOF
			}
		}
	}
}

func (p *Parser) parseEnum(enum *Enum) {
	enum.Type.Enum = enum
	for p.word() != "}" {
		if !p.next() {
			return
		}
	}
	if _, parent := enum.Type.ParentStruct(); parent != nil {
		parent.Inner = append(parent.Inner, enum.Type)
		if !p.next() {
			return
		}
	} else {
		p.file.Enums = append(p.file.Enums, enum)
		if !p.next() {
			if p.err == io.ErrUnexpectedEOF {
				p.err = io.EOF
			}
		}
	}
}

func (p *Parser) parseBits(t *Type) {}

func (p *Parser) parseVariant(variant *Variant) {
	variant.Type.Variant = variant
	for p.word() != "}" {
		if !p.next() {
			return
		}
	}
	if _, parent := variant.Type.ParentStruct(); parent != nil {
		parent.Inner = append(parent.Inner, variant.Type)
		if !p.next() {
			return
		}
	} else {
		p.file.Variants = append(p.file.Variants, variant)
		if !p.next() {
			if p.err == io.ErrUnexpectedEOF {
				p.err = io.EOF
			}
		}
	}
}

func (p *Parser) parseType(t *Type, isAssignable bool) bool {
	if p.err != nil {
		return false
	}

	switch p.word() {
	////////////////////////////////////////////////////////////////////////////////////
	// pointer
	////////////////////////////////////////////////////////////////////////////////////
	case "*":
		t.Pointer = &Pointer{
			Type: &Type{Parent: t},
		}
		t.Code = TypeCodePointer
		if !p.next() {
			return false
		}
		return p.parseType(t.Pointer.Type, false)

		////////////////////////////////////////////////////////////////////////////////////
		// optional
		////////////////////////////////////////////////////////////////////////////////////
	case "?":
		indexType := &Type{}
		*indexType = U8
		t.Optional = &Optional{
			Type:      &Type{Parent: t},
			IndexType: indexType,
		}
		t.Code = TypeCodeOptional
		if !p.next() {
			return false
		}
		return p.parseType(t.Optional.Type, false)

		////////////////////////////////////////////////////////////////////////////////////
		// bool
		////////////////////////////////////////////////////////////////////////////////////
	case "bool", "boolean":
		t.Name = Bool.Name
		t.Code = Bool.Code
		t.Number = Bool.Number

		////////////////////////////////////////////////////////////////////////////////////
		// i8
		////////////////////////////////////////////////////////////////////////////////////
	case "i8":
		t.Name = I8.Name
		t.Code = I8.Code
		t.Number = I8.Number

		////////////////////////////////////////////////////////////////////////////////////
		// u8
		////////////////////////////////////////////////////////////////////////////////////
	case "u8", "byte":
		t.Name = U8.Name
		t.Code = U8.Code
		t.Number = U8.Number

		////////////////////////////////////////////////////////////////////////////////////
		// i16
		////////////////////////////////////////////////////////////////////////////////////
	case "i16", "i16l", "i16le":
		t.Name = I16.Name
		t.Code = I16.Code
		t.Number = I16.Number

	case "i16b", "i16be":
		t.Name = I16B.Name
		t.Code = I16B.Code
		t.Number = I16B.Number

	case "i16n", "i16ne", "int16n", "int16ne":
		t.Name = I16N.Name
		t.Code = I16N.Code
		t.Number = I16N.Number

		////////////////////////////////////////////////////////////////////////////////////
		// u16
		////////////////////////////////////////////////////////////////////////////////////
	case "u16", "u16l", "u16le":
		t.Name = U16.Name
		t.Code = U16.Code
		t.Number = U16.Number

	case "u16b", "u16be":
		t.Name = U16B.Name
		t.Code = U16B.Code
		t.Number = U16B.Number

	case "u16n", "u16ne":
		t.Name = U16N.Name
		t.Code = U16N.Code
		t.Number = U16N.Number

		////////////////////////////////////////////////////////////////////////////////////
		// i32
		////////////////////////////////////////////////////////////////////////////////////
	case "i32", "i32l", "i32le":
		t.Name = I32.Name
		t.Code = I32.Code
		t.Number = I32.Number
	case "i32b", "i32be":
		t.Name = I32B.Name
		t.Code = I32B.Code
		t.Number = I32B.Number
	case "i32n", "i32ne":
		t.Name = I32N.Name
		t.Code = I32N.Code
		t.Number = I32N.Number

		////////////////////////////////////////////////////////////////////////////////////
		// u32
		////////////////////////////////////////////////////////////////////////////////////
	case "u32", "u32l", "u32le":
		t.Name = U32.Name
		t.Code = U32.Code
		t.Number = U32.Number
	case "u32b", "u32be":
		t.Name = U32B.Name
		t.Code = U32B.Code
		t.Number = U32B.Number
	case "u32n", "u32ne":
		t.Name = U32N.Name
		t.Code = U32N.Code
		t.Number = U32N.Number

		////////////////////////////////////////////////////////////////////////////////////
		// i64
		////////////////////////////////////////////////////////////////////////////////////
	case "i64", "i64l", "i64le":
		t.Name = I64.Name
		t.Code = I64.Code
		t.Number = I64.Number
	case "i64b", "i64be":
		t.Name = I64B.Name
		t.Code = I64B.Code
		t.Number = I64B.Number
	case "i64n", "i64ne":
		t.Name = I64N.Name
		t.Code = I64N.Code
		t.Number = I64N.Number

		////////////////////////////////////////////////////////////////////////////////////
		// u64
		////////////////////////////////////////////////////////////////////////////////////
	case "u64", "u64l", "u64le":
		t.Name = U64.Name
		t.Code = U64.Code
		t.Number = U64.Number
	case "u64b", "u64be":
		t.Name = U64B.Name
		t.Code = U64B.Code
		t.Number = U64B.Number
	case "u64n", "u64ne":
		t.Name = U64N.Name
		t.Code = U64N.Code
		t.Number = U64N.Number

		////////////////////////////////////////////////////////////////////////////////////
		// f32
		////////////////////////////////////////////////////////////////////////////////////
	case "f32", "f32l", "f32le":
		t.Name = F32.Name
		t.Code = F32.Code
		t.Number = F32.Number

	case "f32b", "f32be":
		t.Name = F32B.Name
		t.Code = F32B.Code
		t.Number = F32B.Number

	case "f32n", "f32ne":
		t.Name = F32N.Name
		t.Code = F32N.Code
		t.Number = F32N.Number

		////////////////////////////////////////////////////////////////////////////////////
		// f64
		////////////////////////////////////////////////////////////////////////////////////
	case "f64", "f64l", "f64le":
		t.Name = F64.Name
		t.Code = F64.Code
		t.Number = F64.Number

	case "f64b", "f64be":
		t.Name = F64B.Name
		t.Code = F64B.Code
		t.Number = F64B.Number

	case "f64n", "f64ne":
		t.Name = F64N.Name
		t.Code = F64N.Code
		t.Number = F64N.Number

		////////////////////////////////////////////////////////////////////////////////////
		// i128
		////////////////////////////////////////////////////////////////////////////////////
	case "i128", "i128l", "i128le":
		t.Name = I128.Name
		t.Code = I128.Code
		t.Number = I128.Number

	case "i128b", "i128be":
		t.Name = I128B.Name
		t.Code = I128B.Code
		t.Number = I128B.Number

	case "i128n", "i128ne":
		t.Name = I128N.Name
		t.Code = I128N.Code
		t.Number = I128N.Number

		////////////////////////////////////////////////////////////////////////////////////
		// u128
		////////////////////////////////////////////////////////////////////////////////////
	case "u128", "u128l", "u128le":
		t.Name = U128.Name
		t.Code = U128.Code
		t.Number = U128.Number

	case "u128b", "u128be":
		t.Name = U128B.Name
		t.Code = U128B.Code
		t.Number = U128B.Number

	case "u128n", "u128ne":
		t.Name = U128N.Name
		t.Code = U128N.Code
		t.Number = U128N.Number

		////////////////////////////////////////////////////////////////////////////////////
		// string
		////////////////////////////////////////////////////////////////////////////////////
	case "string":

		////////////////////////////////////////////////////////////////////////////////////
		// map
		////////////////////////////////////////////////////////////////////////////////////
	case "map":
		if !p.next() {
			return false
		}
		if p.word() != "<" {
			return p.error("expected '<' after map keyword")
		}
		t.Code = TypeCodeMap
		t.Map = &Map{
			Type: t,
			Kind: TypeCodeMap,
			Key: &Type{
				Parent: t,
			},
			Value: &Type{
				Parent: t,
			},
		}
		if !p.next() {
			return p.error("expected key type for map declaration")
		}
		if !p.parseType(t.Map.Key, false) {
			return false
		}
		if p.word() != "," {
			return p.error("expected ',' after map value declaration")
		}
		if !p.next() {
			return false
		}
		if !p.parseType(t.Map.Value, false) {
			return false
		}
		if p.word() != ">" {
			return p.error("expected '>' after map value declaration")
		}

		////////////////////////////////////////////////////////////////////////////////////
		// ordered map
		////////////////////////////////////////////////////////////////////////////////////
	case "ordered_map":

		////////////////////////////////////////////////////////////////////////////////////
		// tree map
		////////////////////////////////////////////////////////////////////////////////////
	case "tree_map":

		////////////////////////////////////////////////////////////////////////////////////
		// set
		////////////////////////////////////////////////////////////////////////////////////
	case "set":

		////////////////////////////////////////////////////////////////////////////////////
		// ordered_set
		////////////////////////////////////////////////////////////////////////////////////
	case "ordered_set":

		////////////////////////////////////////////////////////////////////////////////////
		// tree_set
		////////////////////////////////////////////////////////////////////////////////////
	case "tree_set":

		////////////////////////////////////////////////////////////////////////////////////
		// vector / array
		////////////////////////////////////////////////////////////////////////////////////
	case "[":
		if !p.nextWord() {
			_ = p.error("incomplete array/vector type declaration")
			return false
		}

		vector := &Vector{Element: &Type{}}
		t.Vector = vector
		vector.Element.Parent = t

		// Is it a vector?
		if p.word() == "]" {
			if !p.nextWord() {
				return p.error("incomplete vector type declaration")
			}
			t.Code = TypeCodeVector
			t.Vector.Kind = TypeCodeVector
			return p.parseType(vector.Element, false)
		}

		length, err := strconv.ParseInt(p.word(), 10, 32)
		if err != nil {
			return p.error("array size must be a positive integer: %s", p.word())
		}
		vector.Length = int(length)
		t.Code = TypeCodeArray
		t.Vector.Kind = TypeCodeArray

		if !p.nextWord() {
			return p.error("incomplete array type declaration")
		}

		return p.parseType(vector.Element, false)

		////////////////////////////////////////////////////////////////////////////////////
		// variant
		////////////////////////////////////////////////////////////////////////////////////
	case "variant":
		if !p.next() {
			return false
		}

		variant := &Variant{Type: t}
		variant.Inline = t.Field
		t.Variant = variant
		t.Code = TypeCodeVariant
		t.Comments = p.flushComments()
		t.Name = ""

		// named struct declaration must have a name
		if t.Field == nil {
			if _, s := t.ParentStruct(); s != nil {
				s.Inner = append(s.Inner, t)
			}
			t.Name = p.word()
			if !p.validateDeclaredName(t.Name, "variant") {
				return false
			}
			if !p.next() {
				return false
			}
		} else {
			if len(t.Comments) == 0 {
				t.Comments = t.Field.Type.Comments
			}
		}

		if p.word() != "{" {
			return p.error("expected '{' for start of struct declaration instead: %s", p.word())
		}
		if !p.next() {
			return false
		}

		p.parseVariant(variant)
		return p.err == nil

		////////////////////////////////////////////////////////////////////////////////////
		// enum
		////////////////////////////////////////////////////////////////////////////////////
	case "enum":
		if !p.next() {
			return false
		}

		// named enum declaration must have a name
		if t.Field == nil {
			t.Name = p.word()
			if !p.validateDeclaredName(t.Name, "variant") {
				return false
			}
			if !p.next() {
				return false
			}
		} else {
			if len(t.Comments) == 0 {
				t.Comments = t.Field.Type.Comments
			}
		}

		// Rust like enum variant declaration?
		if p.word() == "{" {
			if !p.next() {
				return false
			}

			variant := &Variant{Type: t}
			variant.Inline = t.Field
			t.Variant = variant
			t.Code = TypeCodeVariant
			t.Comments = p.flushComments()
			t.Name = ""

			if t.Field == nil {
				if _, s := t.ParentStruct(); s != nil {
					s.Inner = append(s.Inner, t)
				}
			}

			p.parseVariant(variant)
			return p.err == nil
		}

		if p.word() != ":" {
			return p.error("enum requires a ':' followed by an integral type followed by a '{'")
		}

		enum := &Enum{Type: t}
		t.Enum = enum
		t.Code = TypeCodeEnum
		enum.ValueType = &Type{}
		enum.ValueType.Parent = t

		if !p.next() {
			return false
		}

		if !p.parseType(enum.ValueType, false) {
			return false
		}
		if enum.ValueType.Number == nil {
			return p.error("enum cannot extend a non-integral type")
		}
		if p.word() != "{" {
			return p.error("expected '{' for start of struct declaration instead: %s", p.word())
		}
		if !p.next() {
			return false
		}

		p.parseEnum(enum)
		return p.err == nil

		////////////////////////////////////////////////////////////////////////////////////
		// struct
		////////////////////////////////////////////////////////////////////////////////////
	case "union", "struct", "struct16", "struct32", "struct64":
		isUnion := p.word() == "union"

		if !p.next() {
			return false
		}

		s := &Struct{Type: t}
		_, parent := t.ParentStruct()
		s.Parent = parent
		t.Struct = s
		if isUnion {
			t.Code = TypeCodeUnion
		} else {
			t.Code = TypeCodeStruct
		}
		s.Inline = t.Field
		s.Type.Comments = p.flushComments()

		// named struct declaration must have a name
		if t.Field == nil {
			t.Name = p.word()
			if !p.validateDeclaredName(t.Name, "struct") {
				return false
			}
			if !p.next() {
				return false
			}
		} else {
			if len(t.Comments) == 0 {
				t.Comments = t.Field.Type.Comments
			}
		}
		if p.word() != "{" {
			return p.error("expected '{' for start of struct declaration instead: %s", p.word())
		}
		if !p.next() {
			return false
		}

		p.parseStruct(s)
		return p.err == nil

		////////////////////////////////////////////////////////////////////////////////////
		// union
		////////////////////////////////////////////////////////////////////////////////////
	//case "union":
	//	return p.error("unions may only be declared inline within a struct instead use " +
	//		"'variant' type for a type-safe named union")

	default:
		////////////////////////////////////////////////////////////////////////////////////
		// inline string or variant string?
		////////////////////////////////////////////////////////////////////////////////////
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
		t.Name = p.word()
		t.Code = TypeCodeUnknown
	}

	if !p.nextWord() {
		if !p.nextLine() {
			return false
		}
	} else if p.word() == "//" {
		return p.inlineComment(t)
	}

	if !isAssignable {
		return true
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

		// Collapse type to declaring parent
		_, f := t.ParentField()
		if f != nil {
			if !p.parseValue(f.Type) {
				return false
			}
		} else if _, c := t.ParentConst(); c != nil {
			if !p.parseValue(c.Type) {
				return false
			}
		} else {
			return p.error("value declarations may only be declared on const or a struct field")
		}

		if p.word() == "//" {
			return p.inlineComment(t)
		}

		if p.word() == "/*" {
			p.parseSlashStarComment()
			return p.err == nil
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

		return p.error("unexpected text after value declaration: %s", p.word())
	}

	return true
}

func (p *Parser) parseFloatLiteral() (float64, bool) {
	word := ""
	hasDecimal := false

loop:
	for {
		switch p.word() {
		case "'", "_":
			if hasDecimal {
				return 0, false
			}
			if !p.nextWord() {
				break loop
			}
		case ".", ",":
			if hasDecimal {
				return 0, false
			}
			hasDecimal = true
			word += "."
			if !p.nextWord() {
				break loop
			}

		default:
			// is numeral?
			for _, c := range p.word() {
				if c < '0' || c > '9' {
					break loop
				}
			}
			word += p.word()
			if !p.nextWord() {
				break loop
			}
		}
	}

	value, err := strconv.ParseFloat(word, 64)
	if err != nil {
		return 0, false
	}
	return value, true
}

func (p *Parser) parseIntLiteral() (int64, bool) {
	word := ""
	hasSign := false
loop:
	for {
		switch p.word() {
		case "'", "_":
			if !p.nextWord() {
				break loop
			}

		case ";", ",", "/*", "//":
			break loop

		case ".":
			return 0, false

		default:
			// is numeral?
			for _, c := range p.word() {
				if c == '-' {
					if hasSign {
						return 0, false
					}
					hasSign = true
				}
				if c < '0' || c > '9' {
					break loop
				}
			}
			word += p.word()
			if !p.nextWord() {
				break loop
			}
		}
	}

	value, err := strconv.ParseInt(word, 10, 64)
	if err != nil {
		return 0, false
	}
	return value, true
}

func (p *Parser) parseUintLiteral() (uint64, bool) {
	word := ""
loop:
	for {
		switch p.word() {
		case "'", "_":
			if !p.nextWord() {
				break loop
			}

		case ";", ",", "/*", "//":
			break loop

		case ".":
			return 0, false

		default:
			// is numeral?
			for _, c := range p.word() {
				if c == '-' {
					break loop
				}
				if c < '0' || c > '9' {
					break loop
				}
			}
			word += p.word()
			if !p.nextWord() {
				break loop
			}
		}
	}

	value, err := strconv.ParseUint(word, 10, 64)
	if err != nil {
		return 0, false
	}
	return value, true
}

func maybeNumeral(v string) bool {
	if len(v) == 0 {
		return false
	}
	if v[0] >= '0' && v[1] <= '9' {
		return true
	}
	if v[0] == '-' {
		return true
	}
	return false
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
			return p.error("invalid bool value: %s", p.word())
		}
		return true

	case TypeCodeI8, TypeCodeI16, TypeCodeI32, TypeCodeI64:
		switch strings.ToLower(p.word()) {
		case "min":
			t.Value = &Value{Number: t.Number.ToValueSigned(MinNumber(t.Code))}
			return p.next()
		case "max":
			t.Value = &Value{Number: t.Number.ToValueSigned(int64(MaxNumber(t.Code)))}
			return p.next()
		}

		if !maybeNumeral(p.word()) {
			// Const?
			return p.next()
		} else {
			v, success := p.parseIntLiteral()
			if !success {
				return false
			}
			if v < MinNumber(t.Code) || v > int64(MaxNumber(t.Code)) {
				return p.error("int value is out of range")
			}
			t.Value = &Value{Number: t.Number.ToValueSigned(v)}
			return true
		}

	case TypeCodeU8, TypeCodeU16, TypeCodeU32, TypeCodeU64:
		switch strings.ToLower(p.word()) {
		case "min":
			t.Value = &Value{Number: t.Number.ToValue(0)}
			return p.next()
		case "max":
			t.Value = &Value{Number: t.Number.ToValue(MaxNumber(t.Code))}
			return p.next()
		}

		if !maybeNumeral(p.word()) {
			// Const?
			return p.next()
		} else {
			v, success := p.parseUintLiteral()
			if !success {
				return false
			}
			if v > MaxNumber(t.Code) {
				return p.error("uint value is out of range")
			}
			t.Value = &Value{Number: t.Number.ToValue(v)}
			return true
		}

	case TypeCodeF32, TypeCodeF64:
		switch strings.ToLower(p.word()) {
		case "min":
			t.Value = &Value{Number: t.Number.ToValue(0)}
			return p.next()
		case "max":
			t.Value = &Value{Number: t.Number.ToValue(MaxNumber(t.Code))}
			return p.next()
		}

		if !maybeNumeral(p.word()) {
			// Const?
			return p.next()
		} else {
			v, success := p.parseFloatLiteral()
			if !success {
				return false
			}
			if v > math.Float64frombits(MaxNumber(t.Code)) {
				return p.error("floating point value is out of range")
			}
			t.Value = &Value{Number: t.Number.ToValue(math.Float64bits(v))}
			return true
		}

	case TypeCodeEnum:
		if p.word() == "." {
			if !p.next() {
				return false
			}
		}
		t.Value = &Value{Type: t, Unknown: &UnknownValue{Value: p.word()}}
		return p.next()

	case TypeCodeMap, TypeCodeMapOrdered, TypeCodeMapTree, TypeCodeSet, TypeCodeSetOrdered, TypeCodeSetTree:
		value, success := p.parseObjectValue(t)
		if !success {
			return false
		}
		_ = value

	case TypeCodeStruct, TypeCodeUnion:
		value, success := p.parseObjectValue(t)
		if !success {
			return false
		}
		_ = value

	case TypeCodeVariant:
		value, success := p.parseObjectValue(t)
		if !success {
			return false
		}
		_ = value

	case TypeCodePointer:
		switch strings.ToLower(p.word()) {
		case "nil", "null":
			return p.next()
		}
		return p.parseValue(t.Pointer.Type)

	case TypeCodeUnknown:
		// treat as any
	}
	return true
}

// parseObjectValue parses a JSON like object or array like value declaration
func (p *Parser) parseObjectValue(t *Type) (*Object, bool) {
	if p.word() != "{" {
		p.error("expected beginning '{' for value declaration instead: %s", p.word())
		return nil, false
	}

	if !p.next() {
		return nil, false
	}

	object := &Object{Curly: true}

	var success = false
	endCharacter := "}"
	switch p.word() {
	case "{":
	case "[":
		endCharacter = "]"
		object.Curly = false
	default:
		return nil, p.error("expect a value declaration start character '{' or '['")
	}

	if !p.next() {
		return nil, false
	}

	for p.word() != endCharacter {
		prop := &ObjectProperty{
			Index: len(object.Props),
		}

		switch p.word() {
		case "\"", "'":
		case "{":
			prop.HasName = false
			if prop.Value, success = p.parseObjectValue(t); !success {
				return nil, false
			}
		}
		switch p.word() {
		case "\"":
			for p.nextWord() {
				if p.word() == "\"" {
					break
				}
				prop.Name += p.word()
			}
			if p.word() != "\"" {
				p.error("expected a closing \"'\" on the same line")
				return nil, false
			}
			if !p.next() {
				return nil, false
			}

		case "'":
			for p.nextWord() {
				if p.word() == "'\"'" {
					break
				}
				prop.Name += p.word()
			}
			if p.word() != "'" {
				p.error("expected a closing \"'\" on the same line")
				return nil, false
			}
			if !p.next() {
				return nil, false
			}

		default:

		}

		if p.word() == endCharacter {
			// name is the value

			break
		}

		if p.word() == "," {
		}

		if p.word() == ":" {
		}

		object.Props = append(object.Props, prop)
	}

	if !p.next() {
		if t.Parent == nil {
			if p.err == io.ErrUnexpectedEOF {
				p.err = io.EOF
			}
		}
		return nil, false
	}

	return object, true
}

func (p *Parser) isNumber(v string) *NumberValue {
	if len(v) == 0 {
		return nil
	}
	var (
		decimal = false
		sign    = false
		i       int
		c       rune
	)
	for i, c = range v {
		if i == 0 && c == '-' {
			sign = true
			continue
		}
		if c < '0' || c > '9' {
			if c == '.' || c == ',' {
				decimal = true
				i++
				break
			}
			return nil
		}
	}
	if !decimal {
		n := &NumberValue{}
		v, err := strconv.ParseFloat(v, 64)
		if err != nil {
			return nil
		}
		n.Set(*(*uint64)(unsafe.Pointer(&v)))
		return n
	} else {
		n := &NumberValue{}
		if sign {
			v, err := strconv.ParseInt(v, 10, 64)
			if err != nil {
				return nil
			}
			n.Set(*(*uint64)(unsafe.Pointer(&v)))
		} else {
			v, err := strconv.ParseUint(v, 10, 64)
			if err != nil {
				return nil
			}
			n.Set(v)
		}
		return n
	}
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

func (p *Parser) validateDeclaredName(name, kind string) bool {
	if !isValidName(name) {
		_ = p.error("%s name '%s' is not valid", name, kind)
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
	comment := &Comment{
		Star: true,
	}
	begin := p.line
	_ = begin

	if !p.next() {
		return
	}

	start := p.line

	for {
		if p.word() == "*/" {
			text := start.Data[start.Pos.Begin:p.line.Pos.Begin]
			comment.Lines = append(comment.Lines, CommentLine{
				Line: p.line,
				Text: strings.TrimSpace(text),
			})
			p.next()
			return
		}

		if !p.nextWord() {
			text := start.Data[start.ColStart+start.Pos.Begin:]
			comment.Lines = append(comment.Lines, CommentLine{
				Line: p.line,
				Text: strings.TrimSpace(text),
			})

			if !p.nextLine() {
				return
			}

			start = p.line
		}
	}
}
