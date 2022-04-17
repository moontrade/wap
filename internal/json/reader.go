package json

type Reader struct {
	Lexer      Lexer
	Type       uint16
	First      string
	OutOfOrder bool
	IsCompact  bool
	NoFields   bool
}

func OpenReader(b []byte) (Reader, error) {
	r := Reader{
		Lexer: Lexer{
			Data: b,
		},
		Type: 0,
	}

	in := &r.Lexer
	isTopLevel := in.IsStart()
	if in.IsNull() {
		if isTopLevel {
			in.Consumed()
		}
		in.Skip()
		return Reader{}, nil
	}
	in.Delim('{')
	var firstSnapshot Lexer
	snapshot := r.Lexer
LOOP:
	for !in.IsDelim('}') {
		if firstSnapshot.Data == nil {
			firstSnapshot = r.Lexer
			snapshot = r.Lexer
		}
		//snapshot = r.Lexer
		key := in.UnsafeFieldName(false)
		in.WantColon()
		if in.IsNull() {
			in.Skip()
			in.WantComma()
			continue
		}
		switch key {
		case "Type":
			r.Type = in.Uint16()
			if r.OutOfOrder {
				r.Lexer = snapshot
				in.WantComma()
				// Exit loop
				break LOOP
			} else {
				in.WantComma()
				firstSnapshot = r.Lexer
				continue
			}

		case "f", "F":
			if in.IsNull() {
				in.Skip()
				r.NoFields = true
			} else {
				r.IsCompact = true
				if r.Type == 0 {
					r.OutOfOrder = true
					snapshot = r.Lexer
					in.SkipRecursive()
				} else {
					break LOOP
				}
			}

		default:
			if r.Type != 0 {
				r.Lexer = firstSnapshot
				break LOOP
			}
			r.OutOfOrder = true
			in.SkipRecursive()
		}
		in.WantComma()
	}

	if r.OutOfOrder {
		r.Lexer = Lexer{
			Data: b,
		}
		in = &r.Lexer
		in.Delim('{')

		if r.IsCompact && !r.NoFields {
			for in.IsDelim('}') {
				key := in.UnsafeFieldName(false)
				in.WantColon()
				if in.IsNull() {
					in.Skip()
					in.WantComma()
					continue
				}
				switch key {
				case "F":
					in.Delim('[')
					if in.Error() != nil {
						in.Skip()
						return Reader{}, in.Error()
					}
					return r, nil
				}
			}
		}
	}
	return r, nil
}

func (r *Reader) WantComma() {
	r.Lexer.WantComma()
}

func (r *Reader) FieldName() (string, error) {
	if r.Lexer.FatalError != nil {
		return "", r.Lexer.FatalError
	}
	if r.Lexer.IsDelim('}') {
		return "", nil
	}
	name := r.Lexer.UnsafeFieldName(false)
	r.Lexer.WantColon()
	return name, nil
}

func (r *Reader) IsError() bool {
	return r.Lexer.FatalError != nil
}

func (r *Reader) Error() error {
	return r.Lexer.FatalError
}

func (r *Reader) Bool() bool {
	if r.Lexer.IsNull() {
		return false
	}
	return r.Lexer.Bool()
}

func (r *Reader) Int8() int8 {
	if r.Lexer.IsNull() {
		return 0
	}
	return r.Lexer.Int8()
}

func (r *Reader) Uint8() uint8 {
	if r.Lexer.IsNull() {
		return 0
	}
	return r.Lexer.Uint8()
}

func (r *Reader) Int16() int16 {
	if r.Lexer.IsNull() {
		return 0
	}
	return r.Lexer.Int16()
}

func (r *Reader) Uint16() uint16 {
	if r.Lexer.IsNull() {
		return 0
	}
	return r.Lexer.Uint16()
}

func (r *Reader) Int32() int32 {
	if r.Lexer.IsNull() {
		return 0
	}
	return r.Lexer.Int32()
}

func (r *Reader) Uint32() uint32 {
	if r.Lexer.IsNull() {
		return 0
	}
	return r.Lexer.Uint32()
}

func (r *Reader) Int64() int64 {
	if r.Lexer.IsNull() {
		return 0
	}
	return r.Lexer.Int64()
}

func (r *Reader) Uint64() uint64 {
	if r.Lexer.IsNull() {
		return 0
	}
	return r.Lexer.Uint64()
}

func (r *Reader) Float32() float32 {
	if r.Lexer.IsNull() {
		return 0
	}
	return r.Lexer.Float32()
}

func (r *Reader) Float64() float64 {
	if r.Lexer.IsNull() {
		return 0
	}
	return r.Lexer.Float64()
}

func (r *Reader) String() string {
	if r.Lexer.IsNull() {
		return ""
	}
	return r.Lexer.String()
}

/* Compact JSON Messages
HEARTBEAT
MARKET_DATA_SNAPSHOT
MARKET_DATA_SNAPSHOT_INT
MARKET_DATA_UPDATE_TRADE
MARKET_DATA_UPDATE_TRADE_COMPACT
MARKET_DATA_UPDATE_TRADE_INT
MARKET_DATA_UPDATE_LAST_TRADE_SNAPSHOT
MARKET_DATA_UPDATE_BID_ASK
MARKET_DATA_UPDATE_BID_ASK_COMPACT
MARKET_DATA_UPDATE_BID_ASK_INT
MARKET_DATA_UPDATE_SESSION_OPEN
MARKET_DATA_UPDATE_SESSION_OPEN_INT
MARKET_DATA_UPDATE_SESSION_HIGH
MARKET_DATA_UPDATE_SESSION_HIGH_INT
MARKET_DATA_UPDATE_SESSION_LOW
MARKET_DATA_UPDATE_SESSION_LOW_INT
MARKET_DATA_UPDATE_SESSION_VOLUME
MARKET_DATA_UPDATE_OPEN_INTEREST
MARKET_DATA_UPDATE_SESSION_SETTLEMENT
MARKET_DATA_UPDATE_SESSION_SETTLEMENT_INT
MARKET_DEPTH_SNAPSHOT_LEVEL
MARKET_DEPTH_SNAPSHOT_LEVEL_INT
MARKET_DEPTH_UPDATE_LEVEL
MARKET_DEPTH_UPDATE_LEVEL_COMPACT
MARKET_DEPTH_UPDATE_LEVEL_INT
MARKET_DEPTH_FULL_UPDATE_10
MARKET_DEPTH_FULL_UPDATE_20
HISTORICAL_PRICE_DATA_RECORD_RESPONSE
HISTORICAL_PRICE_DATA_TICK_RECORD_RESPONSE
HISTORICAL_PRICE_DATA_RECORD_RESPONSE_INT
HISTORICAL_PRICE_DATA_TICK_RECORD_RESPONSE_INT
*/
