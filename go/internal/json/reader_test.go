package json

import (
	"strconv"
	"testing"
)

func openReaderSuccess(t *testing.T, data string) Reader {
	r, err := OpenReader([]byte(data))
	if err != nil {
		t.Fatal(err)
	}
	return r
}

func assertReader(t *testing.T, r Reader, kind uint16, outOfOrder bool, isCompact bool) {
	if r.Type != kind {
		t.Errorf("%s Type %d != %d", string(r.Lexer.Data), r.Type, kind)
	}
	if r.OutOfOrder != outOfOrder {
		t.Errorf("%s OutOfOrder %s != %s", string(r.Lexer.Data), strconv.FormatBool(r.OutOfOrder), strconv.FormatBool(outOfOrder))
	}
	if r.IsCompact != isCompact {
		t.Errorf("%s IsCompact %s != %s", string(r.Lexer.Data), strconv.FormatBool(r.IsCompact), strconv.FormatBool(isCompact))
	}
}

func TestOpenReader(t *testing.T) {
	assertReader(t, openReaderSuccess(t, "{\"F\": [0,1], \"Type\": 6}"), 6, true, true)
	assertReader(t, openReaderSuccess(t, "{\"Type\": 6, \"F\": [0,1]}"), 6, false, true)

	assertCompact(t, false, "{\"Type\": 6, \"F\": [1, 2,\"ES\"]}")
	assertCompact(t, true, "{\"F\": [1, 2,\"ES\"],  \"Type\": 6}")
}

func assertCompact(t *testing.T, outOfOrder bool, data string) {
	r := openReaderSuccess(t, data)
	if r.OutOfOrder != outOfOrder {
		t.Errorf("%s OutOfOrder %s != %s", string(r.Lexer.Data), strconv.FormatBool(r.OutOfOrder), strconv.FormatBool(outOfOrder))
	}

	elem1 := r.Lexer.Int32()
	r.Lexer.WantComma()
	elem2 := r.Lexer.Int32()
	r.Lexer.WantComma()
	elem3 := r.Lexer.String()
	r.Lexer.WantComma()
	for !r.Lexer.IsDelim(']') {
		r.Lexer.Skip()
		r.Lexer.WantComma()
	}
	if elem1 != 1 {
		t.Fatal("expected 1")
	}
	if elem2 != 2 {
		t.Fatal("expected 2")
	}
	if elem3 != "ES" {
		t.Fatal("expected \"ES\"")
	}
}
