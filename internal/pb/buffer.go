package pb

import (
	"encoding/binary"
	"errors"
	"math"
)

const (
	WireVarint     = 0
	WireFixed32    = 5
	WireFixed64    = 1
	WireBytes      = 2
	WireStartGroup = 3
	WireEndGroup   = 4
)

// EncodeVarint returns the varint encoded bytes of v.
func EncodeVarint(v uint64) []byte {
	return AppendVarint(nil, v)
}

// SizeVarint returns the length of the varint encoded bytes of v.
// This is equal to len(EncodeVarint(v)).
//func SizeVarint(v uint64) int {
//	return SizeVarint(v)
//}

// DecodeVarint parses a varint encoded integer from b,
// returning the integer value and the length of the varint.
// It returns (0, 0) if there is a parse error.
func DecodeVarint(b []byte) (uint64, int) {
	v, n := ConsumeVarint(b)
	if n < 0 {
		return 0, 0
	}
	return v, n
}

// Buffer is a buffer for encoding and decoding the protobuf wire format.
// It may be reused between invocations to reduce memory usage.
type Buffer struct {
	buf           []byte
	err           error
	idx           int
	field         Number
	typ           Type
	deterministic bool
}

// NewBuffer allocates a new Buffer initialized with buf,
// where the contents of buf are considered the unread portion of the buffer.
func NewBuffer(buf []byte) Buffer {
	return Buffer{buf: buf}
}

func NewWriter(buf []byte, typ uint16) Buffer {
	b := Buffer{buf: buf}
	b.WriteHeader(typ)
	return b
}

func (b *Buffer) EOF() bool {
	return b.idx >= len(b.buf)
}

func (b *Buffer) Continue() bool {
	return b.idx >= len(b.buf) && b.err != nil
}

func (b *Buffer) Error() error {
	return b.err
}

// SetDeterministic specifies whether to use deterministic serialization.
//
// Deterministic serialization guarantees that for a given binary, equal
// messages will always be serialized to the same bytes. This implies:
//
//   - Repeated serialization of a message will return the same bytes.
//   - Different processes of the same binary (which may be executing on
//     different machines) will serialize equal messages to the same bytes.
//
// Note that the deterministic serialization is NOT canonical across
// languages. It is not guaranteed to remain stable over time. It is unstable
// across different builds with schema changes due to unknown fields.
// Users who need canonical serialization (e.g., persistent storage in a
// canonical form, fingerprinting, etc.) should define their own
// canonicalization specification and implement their own serializer rather
// than relying on this API.
//
// If deterministic serialization is requested, map entries will be sorted
// by keys in lexographical order. This is an implementation detail and
// subject to change.
func (b *Buffer) SetDeterministic(deterministic bool) {
	b.deterministic = deterministic
}

// SetBuf sets buf as the internal buffer,
// where the contents of buf are considered the unread portion of the buffer.
func (b *Buffer) SetBuf(buf []byte) {
	b.buf = buf
	b.idx = 0
}

// Reset clears the internal buffer of all written and unread data.
func (b *Buffer) Reset() {
	b.buf = b.buf[:0]
	b.idx = 0
}

// Bytes returns the internal buffer.
func (b *Buffer) Bytes() []byte {
	return b.buf
}

// Unread returns the unread portion of the buffer.
func (b *Buffer) Unread() []byte {
	return b.buf[b.idx:]
}

//// Marshal appends the wire-format encoding of m to the buffer.
//func (b *Buffer) Marshal(m Message) error {
//	var err error
//	b.buf, err = marshalAppend(b.buf, m, b.deterministic)
//	return err
//}
//
//// Unmarshal parses the wire-format message in the buffer and
//// places the decoded results in m.
//// It does not reset m before unmarshaling.
//func (b *Buffer) Unmarshal(m Message) error {
//	err := UnmarshalMerge(b.Unread(), m)
//	b.idx = len(b.buf)
//	return err
//}

//type unknownFields struct{ XXX_unrecognized protoimpl.UnknownFields }
//
//func (m *unknownFields) String() string { panic("not implemented") }
//func (m *unknownFields) Reset()         { panic("not implemented") }
//func (m *unknownFields) ProtoMessage()  { panic("not implemented") }

// DebugPrint dumps the encoded bytes of b with a header and footer including s
// to stdout. This is only intended for debugging.
//func (*Buffer) DebugPrint(s string, b []byte) {
//	m := MessageReflect(new(unknownFields))
//	m.SetUnknown(b)
//	b, _ = prototext.MarshalOptions{AllowPartial: true, IndentLine: "\t"}.Marshal(m.Interface())
//	fmt.Printf("==== %s ====\n%s==== %s ====\n", s, b, s)
//}

// EncodeTag encodes num and typ as a varint-encoded tag and appends it to b.
func (b *Buffer) EncodeTag(field Number, typ Type) *Buffer {
	b.buf = AppendVarint(b.buf, EncodeTag(field, typ))
	return b
}

func (b *Buffer) WriteHeader(typ uint16) {
	b.buf = append(b.buf, 0, 0, 0, 0)
	binary.LittleEndian.PutUint16(b.buf[2:], typ)
}

func (b *Buffer) Finish() []byte {
	binary.LittleEndian.PutUint16(b.buf, uint16(len(b.buf)-4))
	return b.buf
}

func (b *Buffer) WriteBool(field Number, v bool) {
	b.EncodeTag(field, VarintType)
	if v {
		b.EncodeVarint(1)
	} else {
		b.EncodeVarint(0)
	}
}

func (b *Buffer) WriteVarint8(field Number, v int8) {
	b.EncodeTag(field, VarintType)
	b.EncodeVarint(uint64(v))
}

func (b *Buffer) WriteZigzag32Int8(field Number, v int8) {
	b.EncodeTag(field, VarintType)
	b.EncodeZigzag32(uint64(v))
}

func (b *Buffer) WriteZigzag64Int8(field Number, v int8) {
	b.EncodeTag(field, VarintType)
	b.EncodeZigzag64(uint64(v))
}

func (b *Buffer) WriteFixed32Int8(field Number, v int8) {
	b.EncodeTag(field, Fixed32Type)
	b.EncodeFixed32(uint64(v))
}

func (b *Buffer) WriteFixed64Int8(field Number, v int8) {
	b.EncodeTag(field, Fixed64Type)
	b.EncodeFixed64(uint64(v))
}

func (b *Buffer) WriteUvarint8(field Number, v uint8) {
	b.EncodeTag(field, VarintType)
	b.EncodeVarint(uint64(v))
}

func (b *Buffer) WriteZigzag32Uint8(field Number, v uint8) {
	b.EncodeTag(field, VarintType)
	b.EncodeZigzag32(uint64(v))
}

func (b *Buffer) WriteZigzag64Uint8(field Number, v uint8) {
	b.EncodeTag(field, VarintType)
	b.EncodeZigzag64(uint64(v))
}

func (b *Buffer) WriteFixed32Uint8(field Number, v uint8) {
	b.EncodeTag(field, Fixed32Type)
	b.EncodeFixed32(uint64(v))
}

func (b *Buffer) WriteFixed64Uint8(field Number, v uint8) {
	b.EncodeTag(field, Fixed64Type)
	b.EncodeFixed64(uint64(v))
}

func (b *Buffer) WriteVarint16(field Number, v int16) {
	b.EncodeTag(field, VarintType)
	b.EncodeVarint(uint64(v))
}

func (b *Buffer) WriteZigzag32Int16(field Number, v int16) {
	b.EncodeTag(field, VarintType)
	b.EncodeZigzag32(uint64(v))
}

func (b *Buffer) WriteZigzag64Int16(field Number, v int16) {
	b.EncodeTag(field, VarintType)
	b.EncodeZigzag64(uint64(v))
}

func (b *Buffer) WriteFixed32Int16(field Number, v int16) {
	b.EncodeTag(field, Fixed32Type)
	b.EncodeFixed32(uint64(v))
}

func (b *Buffer) WriteFixed64Int16(field Number, v int16) {
	b.EncodeTag(field, Fixed64Type)
	b.EncodeFixed64(uint64(v))
}

func (b *Buffer) WriteUvarint16(field Number, v uint16) {
	b.EncodeTag(field, VarintType)
	b.EncodeVarint(uint64(v))
}

func (b *Buffer) WriteZigzag32Uint16(field Number, v uint16) {
	b.EncodeTag(field, VarintType)
	b.EncodeZigzag32(uint64(v))
}

func (b *Buffer) WriteZigzag64Uint16(field Number, v uint16) {
	b.EncodeTag(field, VarintType)
	b.EncodeZigzag64(uint64(v))
}

func (b *Buffer) WriteFixed32Uint16(field Number, v uint16) {
	b.EncodeTag(field, Fixed32Type)
	b.EncodeFixed32(uint64(v))
}

func (b *Buffer) WriteFixed64Uint16(field Number, v uint16) {
	b.EncodeTag(field, Fixed64Type)
	b.EncodeFixed64(uint64(v))
}

func (b *Buffer) WriteVarint32(field Number, v int32) {
	b.EncodeTag(field, VarintType)
	b.EncodeVarint(uint64(v))
}

func (b *Buffer) WriteZigzag32Int32(field Number, v int32) {
	b.EncodeTag(field, VarintType)
	b.EncodeZigzag32(uint64(v))
}

func (b *Buffer) WriteZigzag64Int32(field Number, v int32) {
	b.EncodeTag(field, VarintType)
	b.EncodeZigzag64(uint64(v))
}

func (b *Buffer) WriteFixed32Int32(field Number, v int32) {
	b.EncodeTag(field, Fixed32Type)
	b.EncodeFixed32(uint64(v))
}

func (b *Buffer) WriteFixed64Int32(field Number, v int32) {
	b.EncodeTag(field, Fixed64Type)
	b.EncodeFixed64(uint64(v))
}

func (b *Buffer) WriteUvarint32(field Number, v uint32) {
	b.EncodeTag(field, VarintType)
	b.EncodeVarint(uint64(v))
}

func (b *Buffer) WriteZigzag32Uint32(field Number, v uint32) {
	b.EncodeTag(field, VarintType)
	b.EncodeZigzag32(uint64(v))
}

func (b *Buffer) WriteZigzag64Uint32(field Number, v uint32) {
	b.EncodeTag(field, VarintType)
	b.EncodeZigzag64(uint64(v))
}

func (b *Buffer) WriteFixed32Uint32(field Number, v uint32) {
	b.EncodeTag(field, Fixed32Type)
	b.EncodeFixed32(uint64(v))
}

func (b *Buffer) WriteFixed64Uint32(field Number, v uint32) {
	b.EncodeTag(field, Fixed64Type)
	b.EncodeFixed64(uint64(v))
}

func (b *Buffer) WriteVarint64(field Number, v int64) {
	b.EncodeTag(field, VarintType)
	b.EncodeVarint(uint64(v))
}

func (b *Buffer) WriteZigzag32Int64(field Number, v int64) {
	b.EncodeTag(field, VarintType)
	b.EncodeZigzag32(uint64(v))
}

func (b *Buffer) WriteZigzag64Int64(field Number, v int64) {
	b.EncodeTag(field, VarintType)
	b.EncodeZigzag64(uint64(v))
}

func (b *Buffer) WriteFixed32Int64(field Number, v int64) {
	b.EncodeTag(field, Fixed32Type)
	b.EncodeFixed32(uint64(v))
}

func (b *Buffer) WriteFixed64Int64(field Number, v int64) {
	b.EncodeTag(field, Fixed64Type)
	b.EncodeFixed64(uint64(v))
}

func (b *Buffer) WriteUvarint64(field Number, v uint64) {
	b.EncodeTag(field, VarintType)
	b.EncodeVarint(v)
}

func (b *Buffer) WriteZigzag32Uint64(field Number, v uint64) {
	b.EncodeTag(field, VarintType)
	b.EncodeZigzag32(v)
}

func (b *Buffer) WriteZigzag64Uint64(field Number, v uint64) {
	b.EncodeTag(field, VarintType)
	b.EncodeZigzag64(v)
}

func (b *Buffer) WriteFixed32Uint64(field Number, v uint64) {
	b.EncodeTag(field, Fixed32Type)
	b.EncodeFixed32(v)
}

func (b *Buffer) WriteFixed64Uint64(field Number, v uint64) {
	b.EncodeTag(field, Fixed64Type)
	b.EncodeFixed64(v)
}

func (b *Buffer) WriteVarintFloat32(field Number, v float32) {
	b.WriteUvarint32(field, math.Float32bits(v))
}

func (b *Buffer) WriteZigzag32Float32(field Number, v float32) {
	b.EncodeTag(field, VarintType)
	b.EncodeZigzag32(uint64(math.Float32bits(v)))
}

func (b *Buffer) WriteZigzag64Float32(field Number, v float32) {
	b.EncodeTag(field, VarintType)
	b.EncodeZigzag64(uint64(math.Float32bits(v)))
}

func (b *Buffer) WriteFixed32Float32(field Number, v float32) {
	b.WriteFixed32Uint32(field, math.Float32bits(v))
}

func (b *Buffer) WriteFixed64Float32(field Number, v float32) {
	b.WriteFixed64Uint32(field, math.Float32bits(v))
}

func (b *Buffer) WriteVarintFloat64(field Number, v float64) {
	b.WriteUvarint64(field, math.Float64bits(v))
}

func (b *Buffer) WriteZigzag32Float64(field Number, v float64) {
	b.EncodeTag(field, VarintType)
	b.EncodeZigzag32(math.Float64bits(v))
}

func (b *Buffer) WriteZigzag64Float64(field Number, v float64) {
	b.EncodeTag(field, VarintType)
	b.EncodeZigzag64(math.Float64bits(v))
}

func (b *Buffer) WriteFixed32Float64(field Number, v float64) {
	b.WriteFixed32Uint32(field, math.Float32bits(float32(v)))
}

func (b *Buffer) WriteFixed64Float64(field Number, v float64) {
	b.WriteFixed64Uint64(field, math.Float64bits(v))
}

func (b *Buffer) WriteString(field Number, v string) {
	b.EncodeTag(field, BytesType)
	b.buf = AppendString(b.buf, v)
}

// EncodeVarint appends an unsigned varint encoding to the buffer.
func (b *Buffer) EncodeVarint(v uint64) {
	b.buf = AppendVarint(b.buf, v)
}

// EncodeZigzag32 appends a 32-bit zig-zag varint encoding to the buffer.
func (b *Buffer) EncodeZigzag32(v uint64) {
	b.EncodeVarint(uint64((uint32(v) << 1) ^ uint32((int32(v) >> 31))))
}

// EncodeZigzag64 appends a 64-bit zig-zag varint encoding to the buffer.
func (b *Buffer) EncodeZigzag64(v uint64) {
	b.EncodeVarint(uint64((uint64(v) << 1) ^ uint64((int64(v) >> 63))))
}

// EncodeFixed32 appends a 32-bit little-endian integer to the buffer.
func (b *Buffer) EncodeFixed32(v uint64) {
	b.buf = AppendFixed32(b.buf, uint32(v))
}

// EncodeFixed64 appends a 64-bit little-endian integer to the buffer.
func (b *Buffer) EncodeFixed64(v uint64) {
	b.buf = AppendFixed64(b.buf, uint64(v))
}

// EncodeRawBytes appends a length-prefixed raw bytes to the buffer.
func (b *Buffer) EncodeRawBytes(v []byte) {
	b.buf = AppendBytes(b.buf, v)
}

// EncodeStringBytes appends a length-prefixed raw bytes to the buffer.
// It does not validate whether v contains valid UTF-8.
func (b *Buffer) EncodeStringBytes(v string) {
	b.buf = AppendString(b.buf, v)
}

// EncodeMessage appends a length-prefixed encoded message to the buffer.
//func (b *Buffer) EncodeMessage(m Message) error {
//	var err error
//	b.buf = AppendVarint(b.buf, uint64(Size(m)))
//	b.buf, err = marshalAppend(b.buf, m, b.deterministic)
//	return err
//}

// DecodeTag consumes an encoded tag from the buffer
func (b *Buffer) DecodeTag() (Number, Type, error) {
	var n int
	b.field, b.typ, n = ConsumeTag(b.buf[b.idx:])
	if n < 0 {
		return 0, 0, ParseError(n)
	}
	b.idx += n
	return b.field, b.typ, nil
}

var (
	ErrReadTagFirst   = errors.New("read next tag first")
	ErrExpectedNumber = errors.New("expected number")
	ErrExpectedString = errors.New("expected string")
)

// ReadUint32 reads next encoded number preferring 32-bits
func (b *Buffer) ReadUint32() uint32 {
	if b.field == 0 {
		b.err = ErrReadTagFirst
		return 0
	}
	switch b.typ {
	case VarintType:
		v, err := b.DecodeVarint()
		if err != nil {
			b.err = err
			return 0
		}
		return uint32(v)
	case Fixed64Type:
		v, err := b.DecodeFixed64()
		if err != nil {
			b.err = err
			return 0
		}
		return uint32(v)
	case Fixed32Type:
		v, n := ConsumeFixed32(b.buf[b.idx:])
		if n < 0 {
			b.err = ParseError(n)
			return 0
		}
		b.idx += n
		return v
	default:
		if b.typ == 0 {
			b.err = ErrReadTagFirst
			return 0
		}
		_ = b.ConsumeFieldValue(b.field, b.typ)
		b.err = ErrExpectedNumber
		return 0
	}
}

// ReadUint64 reads next encoded number preferring 64-bits
func (b *Buffer) ReadUint64() uint64 {
	if b.field == 0 {
		b.err = ErrReadTagFirst
		return 0
	}
	switch b.typ {
	case VarintType:
		v, err := b.DecodeVarint()
		if err != nil {
			b.err = err
			return 0
		}
		return v
	case Fixed64Type:
		v, err := b.DecodeFixed64()
		if err != nil {
			b.err = err
			return 0
		}
		return v
	case Fixed32Type:
		v, n := ConsumeFixed32(b.buf[b.idx:])
		if n < 0 {
			b.err = ParseError(n)
			return 0
		}
		b.idx += n
		return uint64(v)
	default:
		if b.typ == 0 {
			b.err = ErrReadTagFirst
			return 0
		}
		_ = b.ConsumeFieldValue(b.field, b.typ)
		b.err = ErrExpectedNumber
		return 0
	}
}

// ReadUint32ZigZag reads next zigzag encoded number preferring 32-bits
func (b *Buffer) ReadUint32ZigZag() uint32 {
	v := b.ReadUint32()
	return uint32((uint32(v) >> 1) ^ uint32((int32(v&1)<<31)>>31))
}

// ReadUint64ZigZag reads next zigzag encoded number preferring 64-bits
func (b *Buffer) ReadUint64ZigZag() uint64 {
	v := b.ReadUint64()
	return uint64((uint64(v) >> 1) ^ uint64((int64(v&1)<<63)>>63))
}

func (b *Buffer) ReadBool() bool {
	v := b.ReadInt32()
	return v != 0
}

// ReadInt8 reads next int8 value
func (b *Buffer) ReadInt8() int8 {
	v := b.ReadInt32()
	return int8(v)
}

// ReadUint8 reads next uint8 value
func (b *Buffer) ReadUint8() uint8 {
	v := b.ReadUint32()
	return uint8(v)
}

// ReadInt16 consumes an encoded unsigned varint from the buffer.
func (b *Buffer) ReadInt16() int16 {
	v := b.ReadUint32()
	return int16(v)
}

// ReadInt16ZigZag reads next zigzag encoded number preferring 32-bits
func (b *Buffer) ReadInt16ZigZag() int16 {
	v := b.ReadUint32ZigZag()
	return int16(v)
}

// ReadUint16 reads next uint16 value
func (b *Buffer) ReadUint16() uint16 {
	v := b.ReadUint32()
	return uint16(v)
}

// ReadUint16ZigZag reads next zigzag encoded number preferring 32-bits
func (b *Buffer) ReadUint16ZigZag() uint16 {
	v := b.ReadUint32ZigZag()
	return uint16(v)
}

// ReadInt32 reads next int32 value
func (b *Buffer) ReadInt32() int32 {
	v := b.ReadUint32()
	return int32(v)
}

// ReadInt32Zigzag reads next zigzag encoded number preferring 32-bits
func (b *Buffer) ReadInt32Zigzag() int32 {
	v := b.ReadUint32ZigZag()
	return int32(v)
}

// ReadInt64 reads next int64 value
func (b *Buffer) ReadInt64() int64 {
	v := b.ReadUint64()
	return int64(v)
}

// ReadInt64ZigZag reads next zigzag encoded number preferring 32-bits
func (b *Buffer) ReadInt64ZigZag() int64 {
	v := b.ReadUint64ZigZag()
	return int64(v)
}

// ReadFloat32 reads next float32 value.
func (b *Buffer) ReadFloat32() float32 {
	v := b.ReadUint32()
	return math.Float32frombits(v)
}

// ReadFloat32ZigZag reads next zigzag encoded number preferring 32-bits
func (b *Buffer) ReadFloat32ZigZag() float32 {
	v := b.ReadUint32ZigZag()
	return math.Float32frombits(v)
}

// ReadFloat64 reads next float64 value
func (b *Buffer) ReadFloat64() float64 {
	v := b.ReadUint64()
	return math.Float64frombits(v)
}

// ReadFloat64ZigZag reads next zigzag encoded number preferring 64-bits
func (b *Buffer) ReadFloat64ZigZag() float64 {
	v := b.ReadUint64ZigZag()
	return math.Float64frombits(v)
}

// ReadString reads next string.
func (b *Buffer) ReadString() string {
	switch b.typ {
	case BytesType:
		v, err := b.DecodeStringBytes()
		if err != nil {
			b.err = err
			return ""
		}
		return v
	default:
		if b.typ == 0 {
			b.err = ErrReadTagFirst
			return ""
		}
		_ = b.ConsumeFieldValue(b.field, b.typ)
		b.err = ErrExpectedString
		return ""
	}
}

// DecodeVarint consumes an encoded unsigned varint from the buffer.
func (b *Buffer) DecodeVarint() (uint64, error) {
	v, n := ConsumeVarint(b.buf[b.idx:])
	if n < 0 {
		return 0, ParseError(n)
	}
	b.idx += n
	return uint64(v), nil
}

// DecodeZigzag32 consumes an encoded 32-bit zig-zag varint from the buffer.
func (b *Buffer) DecodeZigzag32() (uint64, error) {
	v, err := b.DecodeVarint()
	if err != nil {
		return 0, err
	}
	return uint64((uint32(v) >> 1) ^ uint32((int32(v&1)<<31)>>31)), nil
}

// DecodeZigzag64 consumes an encoded 64-bit zig-zag varint from the buffer.
func (b *Buffer) DecodeZigzag64() (uint64, error) {
	v, err := b.DecodeVarint()
	if err != nil {
		return 0, err
	}
	return uint64((uint64(v) >> 1) ^ uint64((int64(v&1)<<63)>>63)), nil
}

// DecodeFixed32 consumes a 32-bit little-endian integer from the buffer.
func (b *Buffer) DecodeFixed32() (uint64, error) {
	v, n := ConsumeFixed32(b.buf[b.idx:])
	if n < 0 {
		return 0, ParseError(n)
	}
	b.idx += n
	return uint64(v), nil
}

// DecodeFixed64 consumes a 64-bit little-endian integer from the buffer.
func (b *Buffer) DecodeFixed64() (uint64, error) {
	v, n := ConsumeFixed64(b.buf[b.idx:])
	if n < 0 {
		return 0, ParseError(n)
	}
	b.idx += n
	return uint64(v), nil
}

// DecodeRawBytes consumes a length-prefixed raw bytes from the buffer.
// If alloc is specified, it returns a copy the raw bytes
// rather than a sub-slice of the buffer.
func (b *Buffer) DecodeRawBytes(alloc bool) ([]byte, error) {
	v, n := ConsumeBytes(b.buf[b.idx:])
	if n < 0 {
		return nil, ParseError(n)
	}
	b.idx += n
	if alloc {
		v = append([]byte(nil), v...)
	}
	return v, nil
}

// DecodeStringBytes consumes a length-prefixed raw bytes from the buffer.
// It does not validate whether the raw bytes contain valid UTF-8.
func (b *Buffer) DecodeStringBytes() (string, error) {
	v, n := ConsumeString(b.buf[b.idx:])
	if n < 0 {
		return "", ParseError(n)
	}
	b.idx += n
	return v, nil
}

func (b *Buffer) Consume() {
	if b.field == 0 {
		return
	}
	n := ConsumeFieldValue(b.field, b.typ, b.buf[b.idx:])
	if n < 0 {
		b.err = ParseError(n)
		return
	}
	b.idx += n
}

func (b *Buffer) ConsumeFieldValue(num Number, t Type) error {
	n := ConsumeFieldValue(num, t, b.buf[b.idx:])
	if n < 0 {
		return ParseError(n)
	}
	b.idx += n
	return nil
}

// DecodeMessage consumes a length-prefixed message from the buffer.
// It does not reset m before unmarshaling.
//func (b *Buffer) DecodeMessage(m Message) error {
//	v, err := b.DecodeRawBytes(false)
//	if err != nil {
//		return err
//	}
//	return UnmarshalMerge(v, m)
//}

// DecodeGroup consumes a message group from the buffer.
// It assumes that the start group marker has already been consumed and
// consumes all bytes until (and including the end group marker).
// It does not reset m before unmarshaling.
//func (b *Buffer) DecodeGroup(m Message) error {
//	v, n, err := consumeGroup(b.buf[b.idx:])
//	if err != nil {
//		return err
//	}
//	b.idx += n
//	return UnmarshalMerge(v, m)
//}

// consumeGroup parses b until it finds an end group marker, returning
// the raw bytes of the message (excluding the end group marker) and the
// the total length of the message (including the end group marker).
func consumeGroup(b []byte) ([]byte, int, error) {
	b0 := b
	depth := 1 // assume this follows a start group marker
	for {
		_, wtyp, tagLen := ConsumeTag(b)
		if tagLen < 0 {
			return nil, 0, ParseError(tagLen)
		}
		b = b[tagLen:]

		var valLen int
		switch wtyp {
		case VarintType:
			_, valLen = ConsumeVarint(b)
		case Fixed32Type:
			_, valLen = ConsumeFixed32(b)
		case Fixed64Type:
			_, valLen = ConsumeFixed64(b)
		case BytesType:
			_, valLen = ConsumeBytes(b)
		case StartGroupType:
			depth++
		case EndGroupType:
			depth--
		default:
			return nil, 0, errors.New("proto: cannot parse reserved wire type")
		}
		if valLen < 0 {
			return nil, 0, ParseError(valLen)
		}
		b = b[valLen:]

		if depth == 0 {
			return b0[:len(b0)-len(b)-tagLen], len(b0) - len(b), nil
		}
	}
}
