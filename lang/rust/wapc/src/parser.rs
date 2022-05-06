use std::borrow::BorrowMut;
use std::convert::Infallible;
use std::error::Error;
use std::fmt;
use std::fmt::{Formatter, Write};
use std::iter::Peekable;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::num::ParseIntError;
use std::ops::{Deref, DerefMut};
use std::str::{CharIndices, FromStr};

use crate::model::{Const, Enum, KindData, Mut, Struct, Type, Union, Variant};

use super::model::{Comments, Module, Position};

pub struct Parser<'a> {
    module: Box<Module<'a>>,
    contents: &'a String,
    iter: Peekable<CharIndices<'a>>,
    mark: Position,
    next_line: usize,
    next_col: usize,
    pos: Position,
    current_char: char,
    prev: Token,
    token: Token,
    next: Token,
    comments: Comments<'a>,
    numeral: Option<Numeral<'a>>,
}

pub enum Numeral<'a> {
    Float { text: &'a str, value: f64 },
    Int { text: &'a str, value: i128 },
    UInt { text: &'a str, value: u128 },
}

pub enum ParseError {
    InvalidCharacter { token: TokenSpan, character: char },
    UnexpectedEOF { token: TokenSpan },
    Expected { found: TokenPosition, expected: Token },
    ExpectedOneOf { found: TokenPosition, one_of: Vec<Token> },
    InvalidFieldNumber { reason: &'static str, float: Option<f64>, int: Option<i128> },
    BadNumeral { token: TokenSpan, reason: String },
}

impl ParseError {
    fn fmt0(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::InvalidCharacter { .. } => Ok(()),
            ParseError::UnexpectedEOF { .. } => Ok(()),
            ParseError::Expected { .. } => Ok(()),
            ParseError::BadNumeral { .. } => Ok(()),
            ParseError::ExpectedOneOf { .. } => Ok(()),
            ParseError::InvalidFieldNumber { .. } => Ok(())
        }
    }
}

impl fmt::Debug for ParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.fmt0(f)
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.fmt0(f)
    }
}

#[derive(Clone, PartialEq)]
pub struct TokenPosition {
    pub token: Token,
    pub start: Position,
    pub end: Position,
}

impl TokenPosition {
    fn new(kind: Token, start: Position, end: Position) -> Self {
        Self { token: kind, start, end }
    }
}

impl TokenPosition {
    fn fmt0(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.token.fmt0(f);
        Ok(())
    }
}

#[derive(Clone, PartialEq)]
pub struct TokenSpan {
    pub token: Token,
    pub start: Position,
    pub end: Position,
    pub slice: String,
}

impl TokenSpan {
    fn new(kind: Token, start: Position, end: Position, slice: String) -> Self {
        Self { token: kind, start, end, slice }
    }
}

impl TokenSpan {
    fn fmt0(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.token.fmt0(f);
        Ok(())
    }
}

#[derive(Copy, Clone, PartialEq)]
pub enum Token {
    None,
    NewLine,
    ForwardSlash,
    BackSlash,
    Period,
    DotDot,
    Star,
    Comma,
    Colon,
    ColonColon,
    Semicolon,
    Pound,
    CurlyOpen,
    CurlyClose,
    At,
    DoubleQuote,
    SingleQuote,
    LessThan,
    GreaterThan,
    BracketOpen,
    BracketClose,
    Carat,
    Ampersand,
    QuestionMark,
    Or,
    Equals,
    Plus,
    Minus,
    Comment,
    CommentMulti,
    Word,
    Numeral,
    EOF,
    UnexpectedEOF,
}

impl Token {
    fn fmt0(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Token::None => f.write_str("None"),
            Token::NewLine => f.write_str("<NEW_LINE>"),
            Token::ForwardSlash => f.write_str("/"),
            Token::BackSlash => f.write_str("\\"),
            Token::Period => f.write_str("."),
            Token::DotDot => f.write_str(".."),
            Token::Star => f.write_str("*"),
            Token::Comma => f.write_str(","),
            Token::Colon => f.write_str(":"),
            Token::ColonColon => f.write_str("::"),
            Token::Semicolon => f.write_str(";"),
            Token::Pound => f.write_str("#"),
            Token::CurlyOpen => f.write_str("{"),
            Token::CurlyClose => f.write_str("}"),
            Token::At => f.write_str("@"),
            Token::DoubleQuote => f.write_str("\""),
            Token::SingleQuote => f.write_str("'"),
            Token::LessThan => f.write_str("<"),
            Token::GreaterThan => f.write_str(">"),
            Token::BracketOpen => f.write_str("["),
            Token::BracketClose => f.write_str("]"),
            Token::Carat => f.write_str("^"),
            Token::Ampersand => f.write_str("&"),
            Token::QuestionMark => f.write_str("?"),
            Token::Or => f.write_str("|"),
            Token::Equals => f.write_str("="),
            Token::Plus => f.write_str("+"),
            Token::Minus => f.write_str("-"),
            Token::Comment => f.write_str("// <COMMENT>"),
            Token::CommentMulti => f.write_str("/* <COMMENT>"),
            Token::Word => f.write_str("<WORD>"),
            Token::Numeral => f.write_str("<NUMERAL>"),
            Token::EOF => f.write_str("<EOF>"),
            Token::UnexpectedEOF => f.write_str("<UNEXPECTED EOF>"),
        };
        Ok(())
    }
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.fmt0(f)
    }
}

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.fmt0(f)
    }
}

pub fn parse(contents: &String) -> anyhow::Result<Box<Module>> {
    Parser::parse(contents)
}

impl<'a> Parser<'a> {
    fn parse(contents: &'a String) -> anyhow::Result<Box<Module<'_>>> {
        let mut p = Self {
            module: Module::new(),
            contents,
            iter: contents.char_indices().peekable(),
            mark: Position::new(0, 0, 0),
            next_line: 1,
            next_col: 1,
            pos: Position::new(1, 1, 0),
            current_char: '\0',
            prev: Token::None,
            token: Token::None,
            next: Token::None,
            comments: Comments::new(),
            numeral: None,
        };
        match p.run() {
            Ok(_) => Ok(p.module),
            Err(r) => Err(r)
        }
    }

    #[inline(always)]
    fn advance(&mut self) -> Option<char> {
        match self.iter.next() {
            None => None,
            Some((index, c)) => {
                if c == '\0' {
                    self.current_char = c;
                    return None;
                }

                self.pos.line = self.next_line;
                self.pos.col = self.next_col;
                self.pos.index = index;

                if c == '\n' {
                    self.next_line += 1;
                    self.next_col = 1;
                } else {
                    self.next_col += 1;
                }
                self.current_char = c;
                Some(c)
            }
        }
    }

    fn set_token(&mut self, token: Token) -> anyhow::Result<()> {
        self.token = token;
        self.mark = self.pos;
        Ok(())
    }

    fn peek(&mut self) -> Option<char> {
        match self.iter.peek() {
            None => None,
            Some((_, ch)) => Some(*ch)
        }
    }

    fn next(&mut self) -> anyhow::Result<()> {
        self.prev = self.token;
        if self.next != Token::None {
            let r = self.next;
            self.next = Token::None;
            return Ok(());
        }
        match self.advance() {
            None => self.set_token(Token::EOF),
            Some(ch) => {
                match ch {
                    ' ' | '\t' | '\r' => self.skip_whitespace(),
                    '\n' => self.set_token(Token::NewLine),
                    '"' => self.set_token(Token::DoubleQuote),
                    '\'' => self.set_token(Token::SingleQuote),
                    '?' => self.set_token(Token::QuestionMark),
                    '{' => self.set_token(Token::CurlyOpen),
                    '/' => match self.peek() {
                        Some(ch) => match ch {
                            '/' => {
                                self.set_token(Token::Comment);
                                self.expect_next_char()?;
                                Ok(())
                            }
                            '*' => {
                                // Multi line
                                self.set_token(Token::CommentMulti);
                                self.expect_next_char()?;
                                Ok(())
                            }
                            _ => {
                                self.set_token(Token::ForwardSlash)
                            }
                        }
                        None => self.set_token(Token::ForwardSlash)
                    }

                    '|' => self.set_token(Token::Or),

                    '-' => match self.peek() {
                        Some(ch) => match ch {
                            '.' | '0' | '1' | '2' | '3' | '4' | '5' | '6' | '7' | '8' | '9' => {
                                self.parse_numeral()
                            }
                            _ => {
                                self.set_token(Token::Minus)
                            }
                        }
                        None => self.set_token(Token::Minus)
                    },

                    '.' => match self.peek() {
                        Some(ch) => match ch {
                            '.' => {
                                self.set_token(Token::DotDot);
                                self.expect_next_char()?;
                                Ok(())
                            }
                            '0' | '1' | '2' | '3' | '4' | '5' | '6' | '7' | '8' | '9' => {
                                self.parse_numeral()
                            }
                            _ => {
                                self.set_token(Token::Period)
                            }
                        }
                        None => self.set_token(Token::Period)
                    },

                    '0' | '1' | '2' | '3' | '4' | '5' | '6' | '7' | '8' | '9' =>
                        self.parse_numeral(),

                    'a' | 'A' | 'b' | 'B' | 'c' | 'C' | 'd' | 'D' | 'e' | 'E' | 'f' |
                    'F' | 'g' | 'G' | 'h' | 'H' | 'i' | 'I' | 'j' | 'J' | 'k' | 'K' |
                    'l' | 'L' | 'm' | 'M' | 'n' | 'N' | 'o' | 'O' | 'p' | 'P' | 'q' |
                    'r' | 'R' | 's' | 'S' | 't' | 'T' | 'u' | 'U' | 'v' | 'V' | 'w' |
                    'W' | 'x' | 'X' | 'y' | 'Y' | 'z' | 'Z' | '_' =>
                        match self.parse_word() {
                            Ok(result) => {
                                Ok(())
                            }
                            Err(reason) => {
                                Err(reason)
                            }
                        }

                    _ => Err(anyhow::Error::msg(ParseError::InvalidCharacter {
                        token: self.token_span(),
                        character: self.current_char,
                    }))
                }
            }
        }
    }

    fn skip_whitespace(&mut self) -> anyhow::Result<()> {
        self.mark = self.pos;

        loop {
            match self.peek() {
                Some(ch) => match ch {
                    ' ' | '\t' | '\r' => {
                        self.expect_next_char()?;
                    }
                    _ => return self.next()
                }
                None => return Ok(())
            }
        }
    }

    fn token_span(&mut self) -> TokenSpan {
        TokenSpan::new(
            self.token,
            self.mark,
            self.pos,
            self.contents[self.mark.index..self.pos.index].to_owned())
    }

    fn token_position(&mut self) -> TokenPosition {
        TokenPosition::new(
            self.token,
            self.mark,
            self.pos)
    }

    fn word(&mut self) -> &'a str {
        self.contents[self.mark.index..=self.pos.index].trim()
    }

    fn expect_next_char(&mut self) -> anyhow::Result<char> {
        match self.advance() {
            None => Err(anyhow::Error::msg(ParseError::UnexpectedEOF {
                token: self.token_span()
            })),
            Some(ch) => Ok(ch)
        }
    }

    fn expect_next(&mut self) -> anyhow::Result<Token> {
        self.next()?;
        Ok(self.token)
    }

    fn unexpected_eof(&mut self) -> anyhow::Result<()> {
        Err(anyhow::Error::msg(ParseError::UnexpectedEOF {
            token: self.token_span()
        }))
    }

    fn err_expected_token(&mut self, expected: Token) -> anyhow::Result<()> {
        Err(anyhow::Error::msg(ParseError::Expected {
            expected,
            found: self.token_position(),
        }))
    }

    fn err_expected_token_one_of(&mut self, one_of: Vec<Token>) -> anyhow::Result<()> {
        Err(anyhow::Error::msg(ParseError::ExpectedOneOf {
            one_of,
            found: self.token_position(),
        }))
    }

    fn expect_token(&mut self, token: Token) -> anyhow::Result<()> {
        if self.token != token {
            Err(anyhow::Error::msg(ParseError::Expected {
                found: self.token_position(),
                expected: token,
            }))
        } else {
            Ok(())
        }
    }

    fn mark(&mut self, token: Token) {
        self.mark = self.pos;
        self.token = token;
    }

    fn parse_word(&mut self) -> anyhow::Result<()> {
        self.mark(Token::Word);
        loop {
            match self.peek() {
                None => return self.unexpected_eof(),
                Some(ch) => {
                    match ch {
                        '0' | '1' | '2' | '3' | '4' | '5' | '6' | '7' | '8' | '9' |
                        'a' | 'A' | 'b' | 'B' | 'c' | 'C' | 'd' | 'D' | 'e' | 'E' | 'f' |
                        'F' | 'g' | 'G' | 'h' | 'H' | 'i' | 'I' | 'j' | 'J' | 'k' | 'K' |
                        'l' | 'L' | 'm' | 'M' | 'n' | 'N' | 'o' | 'O' | 'p' | 'P' | 'q' |
                        'r' | 'R' | 's' | 'S' | 't' | 'T' | 'u' | 'U' | 'v' | 'V' | 'w' |
                        'W' | 'x' | 'X' | 'y' | 'Y' | 'z' | 'Z' | '_' => {}

                        ':' | ',' | '|' => {}
                        ' ' | '\t' => {}

                        '>' => {}

                        _ => {}
                    }
                }
            }
        }
    }

    fn parse_numeral(&mut self) -> anyhow::Result<()> {
        self.mark(Token::Numeral);

        let mut is_float = self.current_char == '.';
        let is_signed = self.current_char == '-';

        loop {
            match self.peek() {
                None => return self.unexpected_eof(),
                Some(ch) => {
                    match ch {
                        '0' | '1' | '2' | '3' | '4' | '5' | '6' | '7' | '8' | '9' => {
                            self.expect_next_char()?;
                        }
                        '.' => {
                            if is_float {
                                return self.try_parse_numeral(is_float, is_signed);
                            } else {
                                is_float = true;
                            }
                        }
                        _ => return self.try_parse_numeral(is_float, is_signed)
                    }
                }
            }
        }
    }

    fn try_parse_numeral(&mut self, is_float: bool, is_signed: bool) -> anyhow::Result<()> {
        self.expect_token(Token::Numeral)?;
        let text = self.word();
        if is_float {
            match f64::from_str(text) {
                Ok(value) => {
                    self.numeral = Some(Numeral::Float { text, value });
                    Ok(())
                }
                Err(err) => Err(anyhow::Error::msg(
                    ParseError::BadNumeral {
                        token: self.token_span(),
                        reason: err.to_string(),
                    }))
            }
        } else if is_signed {
            match i128::from_str(text) {
                Ok(value) => {
                    self.numeral = Some(Numeral::Int { text, value });
                    Ok(())
                }
                Err(err) => Err(anyhow::Error::msg(
                    ParseError::BadNumeral {
                        token: self.token_span(),
                        reason: err.to_string(),
                    }))
            }
        } else {
            match u128::from_str(text) {
                Ok(value) => {
                    self.numeral = Some(Numeral::UInt { text, value });
                    Ok(())
                }
                Err(err) => Err(anyhow::Error::msg(
                    ParseError::BadNumeral {
                        token: self.token_span(),
                        reason: err.to_string(),
                    }))
            }
        }
    }

    fn parse_comment(&mut self) -> anyhow::Result<()> {
        self.expect_token(Token::Comment)?;
        loop {
            match self.advance() {
                None => {
                    self.comments.push(
                        false,
                        self.mark,
                        self.pos,
                        self.contents[self.mark.index..=self.pos.index].trim());
                    return self.set_token(Token::EOF);
                }
                Some(ch) => match ch {
                    '\n' => {
                        self.comments.push(
                            false,
                            self.mark,
                            self.pos,
                            self.contents[self.mark.index..=self.pos.index].trim());
                        return Ok(());
                    }
                    _ => {}
                }
            }
        }
    }

    fn parse_comment_multiline(&mut self) -> anyhow::Result<()> {
        self.expect_token(Token::CommentMulti)?;
        loop {
            match self.expect_next_char()? {
                '\n' => {
                    self.comments.push(true,
                                       self.mark,
                                       self.pos,
                                       self.contents[self.mark.index..=self.pos.index].trim());
                    self.mark = self.pos;
                }
                '*' => {
                    let end = self.pos;
                    match self.expect_next_char()? {
                        '/' => {
                            self.comments.push(
                                true,
                                self.mark,
                                end,
                                self.contents[self.mark.index..=self.pos.index].trim());
                            return Ok(());
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    pub fn run(&mut self) -> anyhow::Result<()> {
        loop {
            match match self.next() {
                Ok(_) => self.token,
                Err(reason) => return Err(reason)
            } {
                Token::EOF => return Ok(()),
                Token::NewLine => {}
                Token::Comment => {
                    self.parse_comment()?;
                }
                Token::CommentMulti => {
                    self.parse_comment_multiline()?;
                }
                Token::Word => {
                    match self.word() {
                        "mod" | "module" => self.parse_mod()?,
                        "import" => {}
                        "type" | "using" => self.parse_alias()?,
                        "const" => self.parse_const()?,
                        "enum" => self.parse_enum()?,

                        "struct" =>
                            Mut::new(self).module.new_struct(|s| {
                                self.expect_next_token(Token::Word)?;
                                s.set_name(self.word().to_owned());
                                self.visit_struct(s)
                            })?,

                        "variant" => self.parse_variant()?,

                        "union" => {
                            Mut::new(self).module.new_union(|u| {
                                self.expect_next_token(Token::Word)?;
                                u.set_name(self.word().to_owned());
                                self.visit_union(u)
                            })?
                        },
                        _ => {}
                    }
                }
                _ => {
                    return self.err_expected_token_one_of(vec![
                        Token::Comment,
                        Token::CommentMulti,
                        Token::NewLine,
                        Token::EOF,
                    ]);
                }
            }
        }
    }

    fn parse_mod(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn parse_alias(&mut self) -> anyhow::Result<()> {
        self.expect_next_token(Token::Word)?;
        let name = self.word();


        Ok(())
    }

    fn parse_const(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn parse_enum(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn visit_const(&mut self, e: &mut Const<'a>) -> anyhow::Result<()> {
        Ok(())
    }

    fn visit_enum(&mut self, e: &mut Enum<'a>) -> anyhow::Result<()> {
        Ok(())
    }

    fn module(&mut self) -> &mut Module<'a> {
        &mut self.module
    }

    fn parse_struct(&mut self) -> anyhow::Result<()> {
        Mut::new(self).module.new_struct(|s| self.visit_struct(s))
    }

    fn visit_struct(&mut self, s: &mut Struct<'a>) -> anyhow::Result<()> {
        // struct <NAME>
        self.expect_next_token(Token::Word)?;
        s.set_name(self.word().to_owned());

        // struct <NAME> {
        self.expect_next_token(Token::CurlyOpen)?;

        'field_loop: loop {
            let mark = self.mark;

            let field_number = match self.consume_comments()? {
                Token::CurlyClose => return Ok(()),

                Token::At => {
                    self.expect_next_token(Token::Numeral)?;
                    let num = self.expect_positive_int()?;
                    self.expect_next_token(Token::Word)?;
                    num
                }

                Token::Numeral => {
                    let num = self.expect_positive_int()?;
                    self.expect_next_token(Token::Word)?;
                    num
                }

                Token::Word => -1i32,

                _ => return self.err_expected_token_one_of(
                    vec![
                        Token::CurlyClose,
                        Token::At,
                        Token::Numeral,
                        Token::Word,
                    ]
                )
            };

            self.consume_comments()?;
            self.expect_token(Token::Word)?;

            let name = self.word();
            match name {
                // Inner type
                "const" => {
                    continue 'field_loop;
                }
                "struct" => {
                    continue 'field_loop;
                }
                "enum" => {
                    continue 'field_loop;
                }
                "variant" => {
                    continue 'field_loop;
                }
                "union" => {
                    continue 'field_loop;
                }

                // Field
                _ => {}
            }

            self.consume_comments()?;

            match self.expect_next()? {
                Token::At => {
                    // Was field number not set?
                    if field_number > -1 {}
                }
                Token::Or => {}
                _ => {}
            }

            if field_number == -1 {}
        }
    }

    fn visit_union(&mut self, u: &mut Union<'a>) -> anyhow::Result<()> {
        Ok(())
    }

    fn visit_variant(&mut self, u: &mut Variant<'a>) -> anyhow::Result<()> {
        Ok(())
    }

    fn parse_variant(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn parse_type(&mut self, parent: &'a mut Type<'a>) -> anyhow::Result<&'a mut Type<'a>> {
        self.consume_comments()?;
        match self.token {
            Token::QuestionMark => {}

            Token::Star => {

            }

            Token::BracketOpen => {

            }

            Token::Word => {
                let name = self.word();
                match name {
                    "bool" => {
                        return Ok(parent.new_bool());
                    }
                    "i8" => {}
                    "i16" | "i16l" => {}
                    "i16b" => {}
                    "i16n" => {}
                    "i32" | "i32l" => {}
                    "i32b" => {}
                    "i32n" => {}
                    "i64" | "i64l" => {}
                    "i64b" => {}
                    "i64n" => {}
                    "i128" | "i128l" => {}
                    "i128b" => {}
                    "i128n" => {}
                    "u8" => {}
                    "u16" | "u16l" => {}
                    "u16b" => {}
                    "u16n" => {}
                    "u32" | "u32l" => {}
                    "u32b" => {}
                    "u32n" => {}
                    "u64" | "u64l" => {}
                    "u64b" => {}
                    "u64n" => {}
                    "u128" | "u128l" => {}
                    "u128b" => {}
                    "u128n" => {}
                    "f32" | "f32l" => {}
                    "f32b" => {}
                    "f32n" => {}
                    "f64" | "f64l" => {}
                    "f64b" => {}
                    "f64n" => {}

                    "map" | "flat_map" => {}
                    "set" | "flat_set" => {}

                    "string" => {}
                    "struct" => {}
                    "union" => {}
                    "variant" => {}
                    "enum" => {}

                    _ => {
                        // user-defined
                    }
                }
            }
            _ => return Err(anyhow::Error::msg(ParseError::ExpectedOneOf {
                one_of: vec![
                    Token::QuestionMark,
                    Token::Star,
                    Token::BracketOpen,
                    Token::Word
                ],
                found: self.token_position(),
            }))
        }
        Ok(parent)
    }

    fn parse_value(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn consume_comments(&mut self) -> anyhow::Result<Token> {
        let mut token = self.token;
        loop {
            match token {
                Token::NewLine => {}
                Token::Comment => self.parse_comment()?,
                Token::CommentMulti => self.parse_comment_multiline()?,
                _ => return Ok(token)
            }
            token = self.expect_next()?;
        }
    }

    fn expect_next_token(&mut self, token: Token) -> anyhow::Result<()> {
        match self.expect_next()? {
            t if t == token => Ok(()),
            _ => self.err_expected_token(token)
        }
    }

    fn expect_next_token_either(&mut self, t1: Token, t2: Token) -> anyhow::Result<Token> {
        let t = self.expect_next()?;
        if t == t1 || t == t2 {
            Ok(t)
        } else {
            Err(anyhow::Error::msg(ParseError::ExpectedOneOf {
                one_of: vec![t1, t2],
                found: self.token_position(),
            }))
        }
    }

    fn expect_next_token_any(&mut self, expected: Vec<Token>) -> anyhow::Result<Token> {
        match self.expect_next()? {
            t if expected.contains(&t) => Ok(t),
            _ => Err(anyhow::Error::msg(ParseError::ExpectedOneOf {
                one_of: expected,
                found: self.token_position(),
            }))
        }
    }

    fn expect_positive_int(&mut self) -> anyhow::Result<i32> {
        self.expect_token(Token::Numeral)?;
        match &self.numeral {
            None => Ok(-1),
            Some(v) => match v {
                Numeral::Float { text, value } => Err(anyhow::Error::msg(ParseError::InvalidFieldNumber {
                    reason: "cannot be a float",
                    float: Some(*value),
                    int: None,
                })),
                Numeral::Int { text, value } => {
                    if *value < 1 {
                        Err(anyhow::Error::msg(ParseError::InvalidFieldNumber {
                            reason: "cannot be less than 1",
                            float: None,
                            int: Some(*value),
                        }))
                    } else {
                        Ok(*value as i32)
                    }
                }
                Numeral::UInt { text, value } => {
                    if *value < 1 {
                        Err(anyhow::Error::msg(ParseError::InvalidFieldNumber {
                            reason: "cannot be less than 1",
                            float: None,
                            int: Some(0),
                        }))
                    } else {
                        Ok(*value as i32)
                    }
                }
            }
        }
    }
}

// macro_rules! expect_next_token_one_of {
//     // The pattern for a single `eval`
//     (eval $e:expr) => {{
//         {
//             let val: usize = $e; // Force types to be integers
//             println!("{} = {}", stringify!{$e}, val);
//         }
//     }};
//
//     // Decompose multiple `eval`s recursively
//     (eval $e:expr, $(eval $es:expr),+) => {{
//         let v = vec![$(eval $es),+];
//         expect_next_token_one_of! { eval $e }
//         expect_next_token_one_of! { $(eval $es),+ }
//     }};
// }

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::model::*;

    use super::*;

    #[test]
    fn test_type() {
        let mut m = Module::new();

        m.new_struct(|t| {
            // t.name = Some(String::from("Order"));
            Ok(())
        });
        // let (mut t, o) = Type::new_struct(Some(String::from("Order")));
        //
        // Type::new_struct_fn(Some(String::from("Order")), |t, s| {
        //
        // });
        //
        // println!("{}", t.name());
        println!("done");
    }

    #[test]
    fn parser() {
        // match std::fs::read_to_string("wapc/src/testdata/s.wap") {
        // let file = std::env::current_dir().unwrap().to_str().unwrap();
        // println!("{}", std::env::current_dir().unwrap().to_str().unwrap());
        match std::fs::read_to_string(Path::new("src/testdata/s.wap")) {
            Ok(s) => {
                match Parser::parse(&s) {
                    Ok(module) => {}
                    Err(reason) => {}
                }
            }
            Err(reason) => {
                println!("{}", reason.to_string());
            }
        }
    }
}