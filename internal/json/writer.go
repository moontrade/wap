package json

import (
	"strconv"
	"unicode/utf8"
)

type Writer struct {
	Data         []byte
	Error        error
	Compact      bool
	NoEscapeHTML bool
}

func NewWriter(b []byte, typ uint16) Writer {
	w := Writer{Data: b}
	w.Data = append(w.Data, "{\"Type\":"...)
	w.Uint16(typ)
	return w
}

func NewCompactWriterInt8(b []byte, typ uint16, first int8) Writer {
	w := Writer{Data: b, Compact: true}
	w.Data = append(w.Data, "{\"Type\":"...)
	w.Uint16(typ)
	w.Data = append(w.Data, ",\"F\":["...)
	w.Int8(first)
	return w
}

func NewCompactWriterUint8(b []byte, typ uint16, first uint8) Writer {
	w := Writer{Data: b, Compact: true}
	w.Data = append(w.Data, "{\"Type\":"...)
	w.Uint16(typ)
	w.Data = append(w.Data, ",\"F\":["...)
	w.Uint8(first)
	return w
}

func NewCompactWriterInt16(b []byte, typ uint16, first int16) Writer {
	w := Writer{Data: b, Compact: true}
	w.Data = append(w.Data, "{\"Type\":"...)
	w.Uint16(typ)
	w.Data = append(w.Data, ",\"F\":["...)
	w.Int16(first)
	return w
}

func NewCompactWriterUint16(b []byte, typ uint16, first uint16) Writer {
	w := Writer{Data: b, Compact: true}
	w.Data = append(w.Data, "{\"Type\":"...)
	w.Uint16(typ)
	w.Data = append(w.Data, ",\"F\":["...)
	w.Uint16(first)
	return w
}

func NewCompactWriterInt32(b []byte, typ uint16, first int32) Writer {
	w := Writer{Data: b, Compact: true}
	w.Data = append(w.Data, "{\"Type\":"...)
	w.Uint16(typ)
	w.Data = append(w.Data, ",\"F\":["...)
	w.Int32(first)
	return w
}

func NewCompactWriterUint32(b []byte, typ uint16, first uint32) Writer {
	w := Writer{Data: b, Compact: true}
	w.Data = append(w.Data, "{\"Type\":"...)
	w.Uint16(typ)
	w.Data = append(w.Data, ",\"F\":["...)
	w.Uint32(first)
	return w
}

func NewCompactWriterInt64(b []byte, typ uint16, first int64) Writer {
	w := Writer{Data: b, Compact: true}
	w.Data = append(w.Data, "{\"Type\":"...)
	w.Uint16(typ)
	w.Data = append(w.Data, ",\"F\":["...)
	w.Int64(first)
	return w
}

func NewCompactWriterUint64(b []byte, typ uint16, first uint64) Writer {
	w := Writer{Data: b, Compact: true}
	w.Data = append(w.Data, "{\"Type\":"...)
	w.Uint16(typ)
	w.Data = append(w.Data, ",\"F\":["...)
	w.Uint64(first)
	return w
}

func NewCompactWriterFloat32(b []byte, typ uint16, first float32) Writer {
	w := Writer{Data: b, Compact: true}
	w.Data = append(w.Data, "{\"Type\":"...)
	w.Uint16(typ)
	w.Data = append(w.Data, ",\"F\":["...)
	w.Float32(first)
	return w
}

func NewCompactWriterFloat64(b []byte, typ uint16, first float64) Writer {
	w := Writer{Data: b, Compact: true}
	w.Data = append(w.Data, "{\"Type\":"...)
	w.Uint16(typ)
	w.Data = append(w.Data, ",\"F\":["...)
	w.Float64(first)
	return w
}

func (w *Writer) Finish() []byte {
	if w.Compact {
		w.Data = append(w.Data, "]}"...)
	} else {
		w.Data = append(w.Data, '}')
	}
	return w.Data
}

// RawByte appends raw binary data to the buffer.
func (w *Writer) RawByte(c byte) {
	w.Data = append(w.Data, c)
}

// RawString appends raw binary data to the buffer.
func (w *Writer) RawString(s string) {
	w.Data = append(w.Data, s...)
}

// Raw appends raw binary data to the buffer or sets the error if it is given. Useful for
// calling with results of MarshalJSONTo-like functions.
func (w *Writer) Raw(data []byte, err error) {
	switch {
	case w.Error != nil:
		return
	case err != nil:
		w.Error = err
	case len(data) > 0:
		w.Data = append(w.Data, data...)
	default:
		w.RawString("null")
	}
}

// RawText encloses raw binary data in quotes and appends in to the buffer.
// Useful for calling with results of MarshalText-like functions.
func (w *Writer) RawText(data []byte, err error) {
	switch {
	case w.Error != nil:
		return
	case err != nil:
		w.Error = err
	case len(data) > 0:
		w.String(string(data))
	default:
		w.RawString("null")
	}
}

// Base64Bytes appends data to the buffer after base64 encoding it
func (w *Writer) Base64Bytes(data []byte) {
	if data == nil {
		w.Data = append(w.Data, "null"...)
		return
	}
	w.Data = append(w.Data, '"')
	w.base64(data)
	w.Data = append(w.Data, '"')
}

func (w *Writer) Uint8(n uint8) {
	w.Data = strconv.AppendUint(w.Data, uint64(n), 10)
}

func (w *Writer) Uint8Field(n string, v uint8) {
	w.RawByte(',')
	w.String(n)
	w.RawByte(':')
	w.Uint8(v)
}

func (w *Writer) Uint8Compact(v uint8) *Writer {
	w.RawByte(',')
	w.Uint8(v)
	return w
}

func (w *Writer) Uint16(n uint16) {
	w.Data = strconv.AppendUint(w.Data, uint64(n), 10)
}

func (w *Writer) Uint16Field(n string, v uint16) {
	w.RawByte(',')
	w.String(n)
	w.RawByte(':')
	w.Uint16(v)
}

func (w *Writer) Uint16Compact(v uint16) *Writer {
	w.RawByte(',')
	w.Uint16(v)
	return w
}

func (w *Writer) Uint32(n uint32) {
	w.Data = strconv.AppendUint(w.Data, uint64(n), 10)
}

func (w *Writer) Uint32Field(n string, v uint32) {
	w.RawByte(',')
	w.String(n)
	w.RawByte(':')
	w.Uint32(v)
}

func (w *Writer) Uint32Compact(v uint32) *Writer {
	w.RawByte(',')
	w.Uint32(v)
	return w
}

func (w *Writer) Uint(n uint) {
	w.Data = strconv.AppendUint(w.Data, uint64(n), 10)
}

func (w *Writer) UintField(n string, v uint) {
	w.RawByte(',')
	w.String(n)
	w.RawByte(':')
	w.Uint(v)
}

func (w *Writer) UintCompact(v uint) *Writer {
	w.RawByte(',')
	w.Uint(v)
	return w
}

func (w *Writer) Uint64(n uint64) {
	w.Data = strconv.AppendUint(w.Data, n, 10)
}

func (w *Writer) Uint64Field(n string, v uint64) {
	w.RawByte(',')
	w.String(n)
	w.RawByte(':')
	w.Uint64(v)
}

func (w *Writer) Uint64Compact(v uint64) *Writer {
	w.RawByte(',')
	w.Uint64(v)
	return w
}

func (w *Writer) Int8(n int8) {
	w.Data = strconv.AppendInt(w.Data, int64(n), 10)
}

func (w *Writer) Int8Field(n string, v int8) {
	w.RawByte(',')
	w.String(n)
	w.RawByte(':')
	w.Int8(v)
}

func (w *Writer) Int8Compact(v int8) *Writer {
	w.RawByte(',')
	w.Int8(v)
	return w
}

func (w *Writer) Int16(n int16) {
	w.Data = strconv.AppendInt(w.Data, int64(n), 10)
}

func (w *Writer) Int16Field(n string, v int16) {
	w.RawByte(',')
	w.String(n)
	w.RawByte(':')
	w.Int16(v)
}

func (w *Writer) Int16Compact(v int16) *Writer {
	w.RawByte(',')
	w.Int16(v)
	return w
}

func (w *Writer) Int32(n int32) {
	w.Data = strconv.AppendInt(w.Data, int64(n), 10)
}

func (w *Writer) Int32Field(n string, v int32) {
	w.RawByte(',')
	w.String(n)
	w.RawByte(':')
	w.Int32(v)
}

func (w *Writer) Int32Compact(v int32) *Writer {
	w.RawByte(',')
	w.Int32(v)
	return w
}

func (w *Writer) Int(n int) {
	w.Data = strconv.AppendInt(w.Data, int64(n), 10)
}

func (w *Writer) IntField(n string, v int) {
	w.RawByte(',')
	w.String(n)
	w.RawByte(':')
	w.Int(v)
}

func (w *Writer) IntCompact(v int) *Writer {
	w.RawByte(',')
	w.Int(v)
	return w
}

func (w *Writer) Int64(n int64) {
	w.Data = strconv.AppendInt(w.Data, n, 10)
}

func (w *Writer) Int64Field(n string, v int64) {
	w.RawByte(',')
	w.String(n)
	w.RawByte(':')
	w.Int64(v)
}

func (w *Writer) Int64Compact(v int64) *Writer {
	w.RawByte(',')
	w.Int64(v)
	return w
}

func (w *Writer) Uint8Str(n uint8) {
	w.Data = append(w.Data, '"')
	w.Data = strconv.AppendUint(w.Data, uint64(n), 10)
	w.Data = append(w.Data, '"')
}

func (w *Writer) Uint16Str(n uint16) {
	w.Data = append(w.Data, '"')
	w.Data = strconv.AppendUint(w.Data, uint64(n), 10)
	w.Data = append(w.Data, '"')
}

func (w *Writer) Uint32Str(n uint32) {
	w.Data = append(w.Data, '"')
	w.Data = strconv.AppendUint(w.Data, uint64(n), 10)
	w.Data = append(w.Data, '"')
}

func (w *Writer) UintStr(n uint) {
	w.Data = append(w.Data, '"')
	w.Data = strconv.AppendUint(w.Data, uint64(n), 10)
	w.Data = append(w.Data, '"')
}

func (w *Writer) Uint64Str(n uint64) {
	w.Data = append(w.Data, '"')
	w.Data = strconv.AppendUint(w.Data, n, 10)
	w.Data = append(w.Data, '"')
}

func (w *Writer) UintptrStr(n uintptr) {
	w.Data = append(w.Data, '"')
	w.Data = strconv.AppendUint(w.Data, uint64(n), 10)
	w.Data = append(w.Data, '"')
}

func (w *Writer) Int8Str(n int8) {
	w.Data = append(w.Data, '"')
	w.Data = strconv.AppendInt(w.Data, int64(n), 10)
	w.Data = append(w.Data, '"')
}

func (w *Writer) Int16Str(n int16) {
	w.Data = append(w.Data, '"')
	w.Data = strconv.AppendInt(w.Data, int64(n), 10)
	w.Data = append(w.Data, '"')
}

func (w *Writer) Int32Str(n int32) {
	w.Data = append(w.Data, '"')
	w.Data = strconv.AppendInt(w.Data, int64(n), 10)
	w.Data = append(w.Data, '"')
}

func (w *Writer) IntStr(n int) {
	w.Data = append(w.Data, '"')
	w.Data = strconv.AppendInt(w.Data, int64(n), 10)
	w.Data = append(w.Data, '"')
}

func (w *Writer) Int64Str(n int64) {
	w.Data = append(w.Data, '"')
	w.Data = strconv.AppendInt(w.Data, n, 10)
	w.Data = append(w.Data, '"')
}

func (w *Writer) Float32(n float32) {
	w.Data = strconv.AppendFloat(w.Data, float64(n), 'g', -1, 32)
}

func (w *Writer) Float32Str(n float32) {
	w.Data = append(w.Data, '"')
	w.Data = strconv.AppendFloat(w.Data, float64(n), 'g', -1, 32)
	w.Data = append(w.Data, '"')
}

func (w *Writer) Float32Field(n string, v float32) {
	w.RawByte(',')
	w.String(n)
	w.RawByte(':')
	w.Float32(v)
}

func (w *Writer) Float32Compact(v float32) *Writer {
	w.RawByte(',')
	w.Float32(v)
	return w
}

func (w *Writer) Float64(n float64) {
	w.Data = strconv.AppendFloat(w.Data, n, 'g', -1, 64)
}

func (w *Writer) Float64Str(n float64) {
	w.Data = append(w.Data, '"')
	w.Data = strconv.AppendFloat(w.Data, n, 'g', -1, 64)
	w.Data = append(w.Data, '"')
}

func (w *Writer) Float64Field(n string, v float64) {
	w.RawByte(',')
	w.String(n)
	w.RawByte(':')
	w.Float64(v)
}

func (w *Writer) Float64Compact(v float64) *Writer {
	w.RawByte(',')
	w.Float64(v)
	return w
}

func (w *Writer) Float64StrField(n string, v float64) {
	w.RawByte(',')
	w.String(n)
	w.RawByte(':')
	w.Float64Str(v)
}

func (w *Writer) Bool(v bool) {
	if v {
		w.Data = append(w.Data, "true"...)
	} else {
		w.Data = append(w.Data, "false"...)
	}
}

func (w *Writer) BoolField(n string, v bool) {
	w.RawByte(',')
	w.String(n)
	w.RawByte(':')
	w.Bool(v)
}

func (w *Writer) BoolCompact(v bool) *Writer {
	w.RawByte(',')
	w.Bool(v)
	return w
}

const chars = "0123456789abcdef"

func getTable(falseValues ...int) [128]bool {
	table := [128]bool{}

	for i := 0; i < 128; i++ {
		table[i] = true
	}

	for _, v := range falseValues {
		table[v] = false
	}

	return table
}

var (
	htmlEscapeTable   = getTable(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, '"', '&', '<', '>', '\\')
	htmlNoEscapeTable = getTable(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, '"', '\\')
)

func (w *Writer) StringField(n string, v string) {
	w.RawByte(',')
	w.String(n)
	w.RawByte(':')
	w.String(v)
}

func (w *Writer) StringCompact(v string) *Writer {
	w.RawByte(',')
	w.String(v)
	return w
}

func (w *Writer) String(s string) {
	w.Data = append(w.Data, '"')

	// Portions of the string that contain no escapes are appended as
	// byte slices.

	p := 0 // last non-escape symbol

	escapeTable := &htmlEscapeTable
	if w.NoEscapeHTML {
		escapeTable = &htmlNoEscapeTable
	}

	for i := 0; i < len(s); {
		c := s[i]

		if c < utf8.RuneSelf {
			if escapeTable[c] {
				// single-width character, no escaping is required
				i++
				continue
			}

			w.Data = append(w.Data, s[p:i]...)
			switch c {
			case '\t':
				w.Data = append(w.Data, '\t')
			case '\r':
				w.Data = append(w.Data, `\r`...)
			case '\n':
				w.Data = append(w.Data, `\n`...)
			case '\\':
				w.Data = append(w.Data, `\\`...)
			case '"':
				w.Data = append(w.Data, `\"`...)
			default:
				w.Data = append(w.Data, `\u00`...)
				w.Data = append(w.Data, chars[c>>4])
				w.Data = append(w.Data, chars[c&0xf])
			}

			i++
			p = i
			continue
		}

		// broken utf
		runeValue, runeWidth := utf8.DecodeRuneInString(s[i:])
		if runeValue == utf8.RuneError && runeWidth == 1 {
			w.Data = append(w.Data, s[p:i]...)
			w.Data = append(w.Data, `\ufffd`...)
			i++
			p = i
			continue
		}

		// jsonp stuff - tab separator and line separator
		if runeValue == '\u2028' || runeValue == '\u2029' {
			w.Data = append(w.Data, s[p:i]...)
			w.Data = append(w.Data, `\u202`...)
			w.Data = append(w.Data, chars[runeValue&0xf])
			i += runeWidth
			p = i
			continue
		}
		i += runeWidth
	}
	w.Data = append(w.Data, s[p:]...)
	w.Data = append(w.Data, '"')
}

const encode = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
const padChar = '='

func (w *Writer) base64(in []byte) {

	if len(in) == 0 {
		return
	}

	//w.Buffer.EnsureSpace(((len(in)-1)/3 + 1) * 4)

	si := 0
	n := (len(in) / 3) * 3

	for si < n {
		// Convert 3x 8bit source bytes into 4 bytes
		val := uint(in[si+0])<<16 | uint(in[si+1])<<8 | uint(in[si+2])

		w.Data = append(w.Data, encode[val>>18&0x3F], encode[val>>12&0x3F], encode[val>>6&0x3F], encode[val&0x3F])

		si += 3
	}

	remain := len(in) - si
	if remain == 0 {
		return
	}

	// Add the remaining small block
	val := uint(in[si+0]) << 16
	if remain == 2 {
		val |= uint(in[si+1]) << 8
	}

	w.Data = append(w.Data, encode[val>>18&0x3F], encode[val>>12&0x3F])

	switch remain {
	case 2:
		w.Data = append(w.Data, encode[val>>6&0x3F], byte(padChar))
	case 1:
		w.Data = append(w.Data, byte(padChar), byte(padChar))
	}
}
