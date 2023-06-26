use std::fmt;
use std::fmt::{Formatter, Write};
use std::iter::Peekable;
use std::num::ParseIntError;
use std::ops::Index;
use std::str::{CharIndices, FromStr};

use crate::model::{Const, Endian, Enum, Kind, KindVariant, Mut, Struct, Type, Value};

use super::model::{Comments, Module, Position};

pub struct Parser<'a> {
    module: Mut<'a, Module<'a>>,
    contents: &'a String,
    iter: Peekable<CharIndices<'a>>,
    mark: Position,
    next_line: usize,
    next_col: usize,
    line_breaks: usize,
    mark_line_breaks: usize,
    pos: Position,
    current_char: char,
    current_line_index: usize,
    prev: Token,
    token: Token,
    next: Token,
    comments: Comments,
    numeral: Option<Numeral>,
}

pub fn parse<'a>(contents: &'a String) -> anyhow::Result<Box<Module<'a>>> {
    Parser::parse(contents)
}

impl<'a> Parser<'a> {
    pub fn parse(contents: &'a String) -> anyhow::Result<Box<Module<'a>>> {
        let mut module = Box::new(Module::new(contents.to_owned()));
        let mut p = Self {
            module: Mut::new(module.as_mut()),
            contents,
            iter: contents.char_indices().peekable(),
            mark: Position::new(0, 0, 0),
            next_line: 1,
            line_breaks: 0,
            mark_line_breaks: 0,
            next_col: 1,
            pos: Position::new(1, 1, 0),
            current_char: '\0',
            current_line_index: 0,
            prev: Token::None,
            token: Token::None,
            next: Token::None,
            comments: Comments::new(),
            numeral: None,
        };
        p.run()?;
        Ok(module)
    }
}

pub enum Numeral {
    Float { text: String, value: f64 },
    Int { text: String, value: i128 },
    UInt { text: String, value: u128 },
}

// impl<'a> Numeral {
//     pub fn to_value(&self) -> Value<'a> {
//         Value::
//     }
// }

pub enum ParseError {
    InvalidCharacter {
        token: TokenSpan,
        character: char,
        message: Option<String>,
    },
    UnexpectedEOF {
        token: TokenSpan,
    },
    Expected {
        found: TokenSpan,
        expected: Token,
    },
    ExpectedOneOf {
        found: TokenSpan,
        one_of: Vec<Token>,
    },
    InvalidFieldNumber {
        reason: &'static str,
        float: Option<f64>,
        int: Option<i128>,
    },
    VariantFieldDefault {
        token: TokenPosition,
    },
    BadNumeral {
        token: TokenSpan,
        reason: String,
    },
    InvalidEnumType {
        token: TokenSpan,
    },
    InvalidConst {
        token: TokenSpan,
        message: Option<String>,
    },
    // CannotNest { parent: &'static str, child: &'static str },
}

impl ParseError {
    fn format_span<'a>(&self, span: &TokenSpan, out: &mut String) -> anyhow::Result<()> {
        let line = format!("line {:}: ", span.start.line);
        out.write_str(line.as_str())?;
        out.write_str(span.line.as_str())?;
        out.write_char('\n')?;
        for _ in 0..line.len() {
            out.write_char(' ')?;
        }
        for _ in 1..span.start.col {
            out.write_char(' ')?;
        }
        for _ in 0..=span.end.col - span.start.col {
            out.write_char('^')?;
        }
        out.write_char('\n')?;
        for _ in 0..line.len() {
            out.write_char(' ')?;
        }
        for _ in 1..span.start.col {
            out.write_char(' ')?;
        }
        out.write_str(span.token.to_string().as_str())?;
        Ok(())
    }

    fn format_pos<'a>(&self, pos: &TokenPosition, out: &mut String) -> anyhow::Result<()> {
        out.write_fmt(format_args!("{:}:{:}", pos.start.line, pos.start.col))?;
        for _ in 1..=pos.start.col {
            out.write_char(' ')?;
        }
        out.write_str(pos.token.to_string().as_str())?;
        Ok(())
    }

    pub fn format(&self) -> anyhow::Result<String> {
        let mut out = String::new();
        match &self {
            ParseError::InvalidCharacter { token, character, message } => {
                out.write_str("invalid character '")?;
                out.write_char(*character)?;
                out.write_str("'")?;
                match *message {
                    Some(ref msg) => {
                        out.write_str(": ")?;
                        out.write_str(msg.as_str())?;
                    }
                    _ => {}
                }
                out.write_str("\n")?;
                self.format_span(token, &mut out)?;
            }
            ParseError::UnexpectedEOF { token } => {
                out.write_str("unexpected EOF")?;
            }
            ParseError::Expected { found, expected } => {
                out.write_str("expected Token: ")?;
                out.write_str(expected.to_chars())?;
                out.write_str("\n")?;
                self.format_span(found, &mut out)?;
            }
            ParseError::ExpectedOneOf { found, one_of } => {
                out.write_str("expected one of the following tokens: ")?;
                for t in one_of {
                    out.write_str(t.to_string().as_str())?;
                    out.write_char(' ')?;
                }
                out.write_str("\n")?;
                self.format_span(found, &mut out)?;
            }
            ParseError::InvalidFieldNumber { reason, float, int } => {}
            ParseError::VariantFieldDefault { token } => {
                out.write_str("union or variant field cannot have a default value")?;
                out.write_str("\n")?;
                self.format_pos(token, &mut out)?;
            }
            ParseError::BadNumeral { token, reason } => {
                out.write_str("bad numeral: ")?;
                out.write_str(reason.as_str())?;
                out.write_str("'\n")?;
                self.format_span(token, &mut out)?;
            } // ParseError::CannotNest { parent, child } => {}
            ParseError::InvalidEnumType { token } => {
                out.write_str("invalid enum type expected <WORD> of '")?;
                out.write_str("i8, i16, i32, i64, i128, u8, u16, u32, u64, u128")?;
                out.write_str("'\n")?;
                self.format_span(token, &mut out)?;
            }
            ParseError::InvalidConst { token, message } => {
                out.write_str("invalid const")?;
                match *message {
                    Some(ref message) => {
                        out.write_str(": ")?;
                        out.write_str(message.as_str())?;
                    }
                    _ => {}
                }
                out.write_str("\n")?;
                self.format_span(token, &mut out)?;
            }
        }

        Ok(out)
    }

    fn fmt0(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self.format() {
            Ok(s) => {
                f.write_str(s.as_str())?;
                Ok(())
            }
            Err(reason) => {
                f.write_str(reason.to_string().as_str())?;
                Ok(())
            }
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
        Self {
            token: kind,
            start,
            end,
        }
    }
}

impl TokenPosition {
    fn fmt0(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.token.fmt0(f)
    }
}

#[derive(Clone, PartialEq)]
pub struct TokenSpan {
    pub token: Token,
    pub start: Position,
    pub end: Position,
    pub line: String,
}

impl TokenSpan {
    fn new(kind: Token, start: Position, end: Position, line: String) -> Self {
        Self {
            token: kind,
            start,
            end,
            line,
        }
    }

    fn fmt0(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(format!("{:}", self.start.line).as_str())?;
        f.write_str(self.line.as_str())?;
        self.token.fmt0(f)
    }
}

#[derive(Copy, Clone, PartialEq)]
pub enum Token {
    None,
    NewLine,
    Whitespace,
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
    LineComment,
    BlockComment,
    Word,
    QualifiedName,
    Numeral,
    EOF,
}

impl Token {
    fn to_chars(&self) -> &'static str {
        match self {
            Token::None => "<<NONE>>",
            Token::NewLine => "<LF>",
            Token::Whitespace => "<WHITESPACE>",
            Token::ForwardSlash => "/",
            Token::BackSlash => "\\",
            Token::Period => ".",
            Token::DotDot => "..",
            Token::Star => "*",
            Token::Comma => ",",
            Token::Colon => ":",
            Token::ColonColon => "::",
            Token::Semicolon => ";",
            Token::Pound => "#",
            Token::CurlyOpen => "{",
            Token::CurlyClose => "}",
            Token::At => "@",
            Token::DoubleQuote => "\"",
            Token::SingleQuote => "'",
            Token::LessThan => "<",
            Token::GreaterThan => ">",
            Token::BracketOpen => "[",
            Token::BracketClose => "]",
            Token::Carat => "^",
            Token::Ampersand => "&",
            Token::QuestionMark => "?",
            Token::Or => "|",
            Token::Equals => "=",
            Token::Plus => "+",
            Token::Minus => "-",
            Token::LineComment => "//",
            Token::BlockComment => "/*",
            Token::Word => "<WORD>",
            Token::QualifiedName => "<QUALIFIED::NAME>",
            Token::Numeral => "<NUMERAL>",
            Token::EOF => "<EOF>",
        }
    }
}

impl Token {
    fn fmt0(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Token::None => f.write_str("<<NONE>>"),
            Token::NewLine => f.write_str("<NEW_LINE>"),
            Token::Whitespace => f.write_str("<WHITESPACE>"),
            Token::ForwardSlash => f.write_str("<FORWARD SLASH>"),
            Token::BackSlash => f.write_str("<BACKSLASH>"),
            Token::Period => f.write_str("<.>"),
            Token::DotDot => f.write_str("<..>"),
            Token::Star => f.write_str("<STAR>"),
            Token::Comma => f.write_str("<COMMA>"),
            Token::Colon => f.write_str("<COLON>"),
            Token::ColonColon => f.write_str("<MODULE::SEPARATOR>"),
            Token::Semicolon => f.write_str("<SEMICOLON>"),
            Token::Pound => f.write_str("<POUND>"),
            Token::CurlyOpen => f.write_str("<CURLYOPEN>"),
            Token::CurlyClose => f.write_str("<CURLYCLOSE>"),
            Token::At => f.write_str("<AT>"),
            Token::DoubleQuote => f.write_str("<DOUBLE QUOTE>"),
            Token::SingleQuote => f.write_str("<SINGLE QUOTE>"),
            Token::LessThan => f.write_str("<LESS THAN>"),
            Token::GreaterThan => f.write_str("<GREATER THAN>"),
            Token::BracketOpen => f.write_str("<OPEN BRACKET>"),
            Token::BracketClose => f.write_str("<CLOSE BRACKET>"),
            Token::Carat => f.write_str("<CARET>"),
            Token::Ampersand => f.write_str("<AND>"),
            Token::QuestionMark => f.write_str("<OPTIONAL>"),
            Token::Or => f.write_str("<OR>"),
            Token::Equals => f.write_str("<EQUALS>"),
            Token::Plus => f.write_str("<PLUS>"),
            Token::Minus => f.write_str("<MINUS>"),
            Token::LineComment => f.write_str("<LINE COMMENT>"),
            Token::BlockComment => f.write_str("<BLOCK COMMENT>"),
            Token::Word => f.write_str("<WORD>"),
            Token::QualifiedName => f.write_str("<QUALIFIED::NAME>"),
            Token::Numeral => f.write_str("<NUMERAL>"),
            Token::EOF => f.write_str("<EOF>"),
        }
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

impl<'a> Parser<'a> {
    fn advance(&mut self) -> Option<char> {
        match self.iter.next() {
            None => None,
            Some((index, c)) => {
                if c == '\0' {
                    self.current_char = c;
                    return None;
                }
                if self.pos.line < self.next_line {
                    self.current_line_index = index;
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
                match c {
                    '\n' => {
                        self.line_breaks += 1;
                    }
                    ' ' | '\t' | '\r' => {}
                    _ => self.line_breaks = 0,
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
            Some((_, ch)) => Some(*ch),
        }
    }

    fn next(&mut self) -> anyhow::Result<()> {
        self.prev = self.token;
        if self.next != Token::None {
            let r = self.next;
            self.next = Token::None;
            return Ok(());
        }
        self.mark_line_breaks = self.line_breaks;
        match self.advance() {
            None => self.set_token(Token::EOF),
            Some(ch) => {
                match ch {
                    ' ' | '\t' | '\r' => loop {
                        self.mark(Token::Whitespace);
                        loop {
                            match self.peek() {
                                Some(c) => match c {
                                    ' ' | '\t' | '\r' => {
                                        self.advance();
                                    }
                                    _ => return Ok(()),
                                },
                                None => return Ok(()),
                            }
                        }
                    },
                    '\n' => self.set_token(Token::NewLine),
                    '\'' => {
                        self.mark(Token::SingleQuote);
                        loop {
                            match self.expect_next_char()? {
                                '\'' => return Ok(()),
                                _ => {}
                            }
                        }
                    }
                    '"' => {
                        self.mark(Token::DoubleQuote);
                        loop {
                            match self.expect_next_char()? {
                                '"' => return Ok(()),
                                _ => {}
                            }
                        }
                    }
                    '?' => self.set_token(Token::QuestionMark),
                    '=' => self.set_token(Token::Equals),
                    // '!' => self.set_token(Token::QuestionMark),
                    '[' => self.set_token(Token::BracketOpen),
                    ']' => self.set_token(Token::BracketClose),
                    '{' => self.set_token(Token::CurlyOpen),
                    '}' => self.set_token(Token::CurlyClose),
                    ',' => self.set_token(Token::Comma),
                    '/' => {
                        self.mark(Token::ForwardSlash);
                        match self.peek() {
                            Some(ch) => match ch {
                                '/' => {
                                    self.token = Token::LineComment;
                                    self.advance();
                                    loop {
                                        match self.advance() {
                                            None => {
                                                self.comments.push(
                                                    false,
                                                    self.mark,
                                                    self.pos,
                                                    self.contents[self.mark.index..=self.pos.index]
                                                        .trim(),
                                                );
                                                return Ok(());
                                            }
                                            Some(ch) => match ch {
                                                '\n' => {
                                                    self.comments.push(
                                                        false,
                                                        self.mark,
                                                        self.pos,
                                                        self.contents
                                                            [self.mark.index..=self.pos.index]
                                                            .trim(),
                                                    );
                                                    return Ok(());
                                                }
                                                _ => {}
                                            },
                                        }
                                    }
                                }
                                '*' => {
                                    self.token = Token::BlockComment;
                                    self.advance();
                                    loop {
                                        match self.expect_next_char()? {
                                            '\n' => {
                                                self.comments.push(
                                                    true,
                                                    self.mark,
                                                    self.pos,
                                                    self.contents[self.mark.index..=self.pos.index]
                                                        .trim(),
                                                );
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
                                                            self.contents
                                                                [self.mark.index..=self.pos.index]
                                                                .trim(),
                                                        );
                                                        return Ok(());
                                                    }
                                                    _ => {}
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                                _ => self.set_token(Token::ForwardSlash),
                            },
                            None => self.set_token(Token::ForwardSlash),
                        }
                    }
                    '|' => self.set_token(Token::Or),
                    ':' => {
                        self.mark(Token::Colon);
                        match self.peek() {
                            Some(ch) => match ch {
                                ':' => {
                                    self.advance();
                                    self.token = Token::ColonColon;
                                    Ok(())
                                }
                                _ => self.set_token(Token::Colon),
                            },
                            None => self.set_token(Token::Colon),
                        }
                    }
                    ';' => self.set_token(Token::Semicolon),
                    '+' => self.set_token(Token::Plus),
                    '-' => match self.peek() {
                        Some(ch) => match ch {
                            '.' | '0'..='9' => self.parse_numeral(),
                            _ => self.set_token(Token::Minus),
                        },
                        None => self.set_token(Token::Minus),
                    },
                    '.' => {
                        self.mark(Token::Period);
                        match self.peek() {
                            Some(ch) => match ch {
                                '.' => {
                                    self.advance();
                                    self.token = Token::DotDot;
                                    Ok(())
                                }
                                '0'..='9' => self.parse_numeral(),
                                _ => self.set_token(Token::Period),
                            },
                            None => self.set_token(Token::Period),
                        }
                    }
                    '@' => self.set_token(Token::At),
                    '0'..='9' => self.parse_numeral(),
                    'a'..='z' | 'A'..='Z' | '_' => {
                        self.mark(Token::Word);
                        loop {
                            match self.peek() {
                                None => return self.unexpected_eof(),
                                Some(ch) => match ch {
                                    '0'..='9' | 'a'..='z' | 'A'..='Z' | '_' => {
                                        self.advance();
                                    }
                                    ':' => match self.peek() {
                                        Some(':') => {
                                            self.advance();
                                            self.advance();
                                            self.token = Token::QualifiedName;
                                        }
                                        Some(_) => return Ok(()),
                                        None => return Ok(()),
                                    },
                                    _ => return Ok(()),
                                },
                            }
                        }
                    }

                    _ => Err(anyhow::Error::msg(ParseError::InvalidCharacter {
                        token: self.token_span(),
                        character: self.current_char,
                        message: None,
                    })),
                }
            }
        }
    }

    fn token_span(&mut self) -> TokenSpan {
        let new_line_index = self.contents[self.pos.index..self.contents.len()].find('\n');
        let line = match new_line_index {
            Some(index) => {
                self.contents[self.current_line_index..(self.pos.index + index)].to_owned()
            }
            None => {
                self.contents[self.current_line_index..=self.pos.index].to_owned()
            }
        };

        // let slice = self.contents[self.current_line_index..=self.pos.index].to_owned();

        TokenSpan::new(
            self.token,
            self.mark,
            self.pos,
            line,
        )
    }

    fn token_position(&mut self) -> TokenPosition {
        TokenPosition::new(self.token, self.mark, self.pos)
    }

    fn word(&self) -> &'a str {
        self.contents[self.mark.index..=self.pos.index].trim()
    }

    fn expect_next_char(&mut self) -> anyhow::Result<char> {
        match self.advance() {
            None => Err(anyhow::Error::msg(ParseError::UnexpectedEOF {
                token: self.token_span(),
            })),
            Some(ch) => Ok(ch),
        }
    }

    fn expect_next(&mut self) -> anyhow::Result<Token> {
        self.next()?;
        if self.token == Token::EOF {
            Err(anyhow::Error::msg(ParseError::UnexpectedEOF {
                token: self.token_span(),
            }))
        } else {
            Ok(self.token)
        }
    }

    fn expect_next_token(&mut self, token: Token) -> anyhow::Result<()> {
        match self.expect_next()? {
            t if t == token => Ok(()),
            _ => self.err_expected_token(token),
        }
    }

    fn expect_next_token_any(&mut self, expected: Vec<Token>) -> anyhow::Result<Token> {
        match self.expect_next()? {
            t if expected.contains(&t) => Ok(t),
            _ => Err(anyhow::Error::msg(ParseError::ExpectedOneOf {
                one_of: expected,
                found: self.token_span(),
            })),
        }
    }

    fn unexpected_eof(&mut self) -> anyhow::Result<()> {
        Err(anyhow::Error::msg(ParseError::UnexpectedEOF {
            token: self.token_span(),
        }))
    }

    fn err_expected_token(&mut self, expected: Token) -> anyhow::Result<()> {
        Err(anyhow::Error::msg(ParseError::Expected {
            expected,
            found: self.token_span(),
        }))
    }

    fn err_expected_token_one_of(&mut self, one_of: Vec<Token>) -> anyhow::Result<()> {
        Err(anyhow::Error::msg(ParseError::ExpectedOneOf {
            one_of,
            found: self.token_span(),
        }))
    }

    fn err_token_one_of<T>(&mut self, one_of: Vec<Token>) -> anyhow::Result<T> {
        Err(anyhow::Error::msg(ParseError::ExpectedOneOf {
            one_of,
            found: self.token_span(),
        }))
    }

    fn expect_token(&mut self, token: Token) -> anyhow::Result<()> {
        if self.token != token {
            Err(anyhow::Error::msg(ParseError::Expected {
                found: self.token_span(),
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

    fn parse_numeral(&mut self) -> anyhow::Result<()> {
        self.mark(Token::Numeral);

        let mut is_float = self.current_char == '.';
        let is_signed = self.current_char == '-';

        loop {
            match self.peek() {
                None => return self.unexpected_eof(),
                Some(ch) => match ch {
                    '0'..='9' => {
                        self.expect_next_char()?;
                    }
                    '.' => {
                        if is_float {
                            return self.try_parse_numeral(is_float, is_signed);
                        } else {
                            is_float = true;
                        }
                    }
                    _ => return self.try_parse_numeral(is_float, is_signed),
                },
            }
        }
    }

    fn parse_numeral_and_take(&mut self) -> anyhow::Result<Numeral> {
        self.parse_numeral()?;
        Ok(self.numeral.take().unwrap())
    }

    fn try_parse_numeral(&mut self, is_float: bool, is_signed: bool) -> anyhow::Result<()> {
        self.expect_token(Token::Numeral)?;
        let text = self.word();
        if is_float {
            match f64::from_str(text) {
                Ok(value) => {
                    self.numeral = Some(Numeral::Float { text: text.to_owned(), value });
                    Ok(())
                }
                Err(err) => Err(anyhow::Error::msg(ParseError::BadNumeral {
                    token: self.token_span(),
                    reason: err.to_string(),
                })),
            }
        } else if is_signed {
            match i128::from_str(text) {
                Ok(value) => {
                    self.numeral = Some(Numeral::Int { text: text.to_owned(), value });
                    Ok(())
                }
                Err(err) => Err(anyhow::Error::msg(ParseError::BadNumeral {
                    token: self.token_span(),
                    reason: err.to_string(),
                })),
            }
        } else {
            match u128::from_str(text) {
                Ok(value) => {
                    self.numeral = Some(Numeral::UInt { text: text.to_owned(), value });
                    Ok(())
                }
                Err(err) => Err(anyhow::Error::msg(ParseError::BadNumeral {
                    token: self.token_span(),
                    reason: err.to_string(),
                })),
            }
        }
    }

    fn is_non_essential(&self) -> bool {
        match self.token {
            Token::Whitespace | Token::NewLine | Token::LineComment | Token::BlockComment => true,
            _ => false
        }
    }

    fn maybe_consume_non_essential(&mut self) -> anyhow::Result<Token> {
        loop {
            match self.token {
                Token::Whitespace | Token::NewLine | Token::LineComment | Token::BlockComment => {
                    self.expect_next()?;
                }
                _ => return Ok(self.token),
            }
        }
    }

    fn next_essential(&mut self) -> anyhow::Result<Token> {
        loop {
            match self.expect_next()? {
                Token::Whitespace | Token::NewLine | Token::LineComment | Token::BlockComment => {}
                _ => return Ok(self.token),
            }
        }
    }

    fn expect_curly_open(&mut self) -> anyhow::Result<Token> {
        let mut new_line_count = 0;
        loop {
            match self.expect_next()? {
                Token::NewLine => {
                    new_line_count += 1;
                    if new_line_count > 1 {
                        self.expect_token(Token::CurlyOpen)?;
                    }
                }
                Token::Whitespace => {}
                Token::LineComment | Token::BlockComment => {}
                Token::CurlyOpen => {
                    self.next_essential()?;
                    return Ok(self.token);
                }
                _ => {
                    return Err(anyhow::Error::msg(ParseError::Expected {
                        expected: Token::CurlyOpen,
                        found: self.token_span(),
                    }));
                }
            }
        }
    }

    fn expect_colon_curly_open(&mut self) -> anyhow::Result<Token> {
        let mut new_line_count = 0;
        loop {
            match self.expect_next()? {
                Token::NewLine => {
                    new_line_count += 1;
                    if new_line_count > 1 {
                        if self.token != Token::CurlyOpen && self.token != Token::Colon {
                            return Err(anyhow::Error::msg(ParseError::ExpectedOneOf {
                                one_of: vec![Token::Colon, Token::CurlyOpen],
                                found: self.token_span(),
                            }));
                        }
                    }
                }
                Token::Whitespace => {}
                Token::LineComment | Token::BlockComment => {}
                Token::Colon => {
                    self.next_essential()?;
                    return Ok(self.token);
                }
                Token::CurlyOpen => {
                    return Ok(self.token);
                }
                _ => {
                    return Err(anyhow::Error::msg(ParseError::ExpectedOneOf {
                        one_of: vec![Token::Colon, Token::CurlyOpen],
                        found: self.token_span(),
                    }));
                }
            }
        }
    }

    fn finish_line(&mut self) -> anyhow::Result<()> {
        loop {
            self.next()?;
            match self.token {
                Token::Whitespace => {}
                Token::NewLine => return Ok(()),
                Token::LineComment => return Ok(()),
                _ => {
                    return self.err_expected_token_one_of(vec![
                        Token::Whitespace,
                        Token::NewLine,
                        Token::LineComment,
                    ]);
                }
            }
        }
    }
}

impl<'a> Parser<'a> {
    pub fn run(&mut self) -> anyhow::Result<()> {
        let s = unsafe { &mut *(self as *mut Self) };
        loop {
            match match self.next() {
                Ok(_) => self.token,
                Err(reason) => return Err(reason),
            } {
                Token::EOF => return Ok(()),
                Token::NewLine | Token::Whitespace => {}
                Token::LineComment => {}
                Token::BlockComment => {}
                Token::Word => {
                    let text = self.word();
                    match text {
                        "mod" | "module" | "namespace" => self.parse_mod()?,
                        "import" => {}
                        "type" | "using" | "alias" => self.parse_alias()?,
                        "const" => self.parse_const()?,
                        "enum" => {
                            self.next_essential()?;
                            s.module_mut(|m| {
                                self.parse_enum(m.root_mut(), false)
                            })?;
                            self.maybe_consume_non_essential()?;
                        }
                        "struct" => {
                            self.next_essential()?;
                            self.expect_token(Token::Word)?;
                            let s = self.module_mut(|m| Ok(m.new_struct()))?;
                            s.set_name(self.word().to_owned());
                            self.expect_curly_open()?;
                            self.visit_struct(s)?;
                        }
                        _ => {}
                    }
                }
                _ => {
                    return self.err_expected_token_one_of(vec![
                        Token::LineComment,
                        Token::BlockComment,
                        Token::NewLine,
                        Token::EOF,
                    ]);
                }
            }
        }
    }

    fn parse_mod(&mut self) -> anyhow::Result<()> {
        self.expect_next_token(Token::Whitespace)?;
        match self.expect_next()? {
            Token::Word | Token::QualifiedName => {
                self.module.root_mut().module_mut().full_name = self.word().to_owned();
            }
            _ => return self.err_expected_token_one_of(vec![Token::Word, Token::ColonColon]),
        }
        loop {
            self.next()?;
            match self.token {
                Token::Whitespace => {}
                Token::NewLine => return Ok(()),
                Token::LineComment => return Ok(()),
                Token::EOF => return Ok(()),
                _ => {
                    return self.err_expected_token_one_of(vec![
                        Token::Whitespace,
                        Token::NewLine,
                        Token::LineComment,
                        Token::EOF,
                    ]);
                }
            }
        }
    }

    fn parse_alias(&mut self) -> anyhow::Result<()> {
        self.next_essential()?;
        self.expect_token(Token::Word)?;

        let name = self.word();
        let alias = self.module_mut(|m| Ok(m.new_alias(name, None)))?;

        self.next_essential()?;
        // Consume '=' if exists
        if self.token == Token::Equals {
            self.expect_next()?;
            self.next_essential()?;
        }

        // let alias = self.module.new_alias(name, None);
        let of = self.parse_type(alias.this_mut())?;
        alias.set_of(Some(of));

        Ok(())
    }

    fn parse_const(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn parse_enum(
        &mut self,
        parent: &'a mut Type<'a>,
        inline: bool,
    ) -> anyhow::Result<(&'a mut Enum<'a>, &'a mut Type<'a>)> {
        self.maybe_consume_non_essential()?;
        let comments = self.comments.take();

        let name = if !inline {
            let word = self.word();
            self.expect_next()?;
            word
        } else {
            ""
        };

        let (e, parent) = parent.new_enum(name, None, comments);
        // let mut e = self.module_mut(move |m| m.new_enum(name, None, comments));

        self.maybe_consume_non_essential()?;

        if self.token == Token::Colon {
            self.next_essential()?;
        }

        self.expect_token(Token::Word)?;
        let span = self.token_span();
        let of = self.parse_type(e.this_mut())?;
        match of.kind() {
            Kind::I8 => {}
            Kind::U8 => {}
            Kind::I16(_) => {}
            Kind::U16(_) => {}
            Kind::I32(_) => {}
            Kind::U32(_) => {}
            Kind::I64(_) => {}
            Kind::U64(_) => {}
            Kind::I128(_) => {}
            Kind::U128(_) => {}
            Kind::F32(_) => {}
            Kind::F64(_) => {}
            _ => {
                return Err(anyhow::Error::msg(ParseError::InvalidEnumType { token: span }));
            }
        }
        e.set_of(Some(of));
        self.next_essential()?;

        self.expect_token(Token::CurlyOpen)?;
        self.expect_next()?;
        self.visit_enum(e)?;
        Ok((e, parent))
    }

    fn visit_const(&mut self, e: &mut Const<'a>) -> anyhow::Result<()> {
        Ok(())
    }

    fn visit_enum(&mut self, e: &mut Enum<'a>) -> anyhow::Result<()> {
        'option_loop: loop {
            self.maybe_consume_non_essential()?;
            let comments = self.comments.take();

            if self.token == Token::CurlyClose {
                return Ok(());
            }

            self.expect_token(Token::Word)?;
            let name = self.word();

            self.next_essential()?;
            if self.token == Token::Equals {
                self.next_essential()?;
            }

            self.expect_token(Token::Numeral)?;
            let numeral = self.parse_numeral_and_take()?;
            let value = match numeral {
                Numeral::Float { text, value } => Value::F64(value),
                Numeral::Int { text, value } => Value::I128(value),
                Numeral::UInt { text, value } => Value::U128(value)
            };

            self.expect_next()?;
            if self.token == Token::Whitespace {
                self.expect_next()?;
            }

            let line_comment = self.comments.take();
            e.add_option(name, value, comments, line_comment);

            if self.token == Token::Comma {
                self.expect_next()?;
            }

            if self.token == Token::CurlyClose {
                return Ok(());
            }
        }
    }

    fn module_mut<R>(&mut self, f: impl FnOnce(&'a mut Module<'a>) -> anyhow::Result<R>) -> anyhow::Result<R> {
        f(self.module.as_mut())
    }

    fn visit_struct(&mut self, s: &mut Struct<'a>) -> anyhow::Result<()> {
        self.maybe_consume_non_essential()?;
        if self.token == Token::CurlyClose {
            return Ok(());
        }

        let mut counter = 1;

        'field_loop: loop {
            self.maybe_consume_non_essential()?;
            let comments = self.comments.take();
            let mut field_number = match self.token {
                Token::Whitespace | Token::NewLine | Token::LineComment | Token::BlockComment => {
                    self.expect_next()?;
                    continue 'field_loop;
                }
                Token::CurlyClose => return Ok(()),

                Token::At => {
                    self.expect_next_token(Token::Numeral)?;
                    let num = self.expect_positive_int()?;
                    self.next_essential()?;
                    num
                }

                Token::Numeral => {
                    let num = self.expect_positive_int()?;
                    self.next_essential()?;
                    num
                }

                Token::Word => 0usize,

                Token::DotDot => {
                    self.next_essential()?;
                    self.expect_token(Token::Numeral)?;
                    let size = self.expect_positive_int()?;
                    self.next_essential()?;
                    let mut field = s.new_field("", 0, comments);
                    field.this_mut().new_padding(size);
                    continue 'field_loop;
                }

                _ => {
                    return self.err_expected_token_one_of(vec![
                        Token::CurlyClose,
                        Token::At,
                        Token::Numeral,
                        Token::Word,
                    ]);
                }
            };

            let mut field_number_generated = false;
            if field_number == 0 {
                field_number = counter;
                field_number_generated = true;
            }
            counter = counter + 1;

            self.maybe_consume_non_essential()?;
            self.expect_token(Token::Word)?;

            let name = self.word();
            match name {
                // Inner type
                "const" => {
                    self.next_essential()?;
                    self.parse_const()?;
                    continue 'field_loop;
                }
                "struct" => {
                    self.next_essential()?;
                    continue 'field_loop;
                }
                "enum" => {
                    self.next_essential()?;
                    continue 'field_loop;
                }
                "variant" => {
                    self.next_essential()?;
                    continue 'field_loop;
                }
                "union" => {
                    self.next_essential()?;
                    continue 'field_loop;
                }

                // Field
                _ => {}
            }

            self.next_essential()?;

            let field = s.new_field(name, field_number, comments);

            let ty = self.parse_type(field.this_mut())?;
            field.set_type(Some(ty));
            self.maybe_consume_non_essential()?;
        }
    }

    fn parse_type(&mut self, parent: &'a mut Type<'a>) -> anyhow::Result<&'a mut Type<'a>> {
        let _is_const = parent.is_const();
        let _is_alias = parent.is_alias();
        let is_field = parent.is_field();
        let _is_root = parent.is_module();

        // self.consume_non_essential()?;
        let (new_type, parent) = match self.token {
            Token::QuestionMark => {
                if parent.is_optional() {
                    return Err(anyhow::Error::msg(ParseError::InvalidCharacter {
                        token: self.token_span(),
                        character: self.current_char,
                        message: Some("cannot have an optional of an optional \'??\' can only have a single '?' character".to_owned()),
                    }));
                }
                self.expect_next()?;
                let optional = parent.new_optional(None);
                let child = self.parse_type(optional.this_mut())?;
                optional.set_of(Some(child));
                (optional.this_mut(), parent)
            }

            Token::Star => {
                self.expect_next()?;
                let pointer = parent.new_pointer(None);
                let child = self.parse_type(pointer.this_mut())?;
                pointer.set_of(Some(child));
                (pointer.this_mut(), parent)
            }

            Token::BracketOpen => {
                match self.expect_next()? {
                    // array
                    Token::Numeral => {
                        let size = self.expect_positive_int()?;
                        match self.expect_next()? {
                            Token::BracketClose => {
                                let new_type = parent.new_array(size);
                                self.next_essential()?;
                                let child = self.parse_type(new_type.this_mut())?;
                                new_type.set_element(Some(child));
                                (new_type.this_mut(), parent)
                            }
                            Token::DotDot => {
                                self.expect_next_token(Token::BracketClose)?;
                                let new_type = parent.new_array_vector(size);
                                self.next_essential()?;
                                let child = self.parse_type(new_type.this_mut())?;
                                new_type.set_element(Some(child));
                                (new_type.this_mut(), parent)
                            }
                            _ => {
                                return self
                                    .err_token_one_of(vec![Token::Numeral, Token::BracketClose]);
                            }
                        }
                    }
                    // vector
                    Token::BracketClose => {
                        let new_type = parent.new_vector();
                        self.next_essential()?;
                        let child = self.parse_type(new_type.this_mut())?;
                        new_type.set_element(Some(child));
                        (new_type.this_mut(), parent)
                    }
                    _ => return self.err_token_one_of(vec![Token::Numeral, Token::BracketClose]),
                }
            }

            Token::QualifiedName => {
                // user-defined
                (parent.new_unknown(Some(self.word())).this_mut(), parent)
            }

            Token::Word => {
                let name = self.word();
                match name {
                    "bool" | "boolean" => {
                        let t = unsafe {
                            &mut *(parent.new_bool() as *mut Type<'a>)
                        };
                        (t, parent)
                    }
                    "i8" => {
                        let t = unsafe {
                            &mut *(parent.new_i8() as *mut Type<'a>)
                        };
                        (t, parent)
                    }
                    "i16" | "i16l" => (parent.new_i16(Endian::Little).this_mut(), parent),
                    "i16b" => (parent.new_i16(Endian::Big).this_mut(), parent),
                    "i16n" => (parent.new_i16(Endian::Native).this_mut(), parent),
                    "i32" | "i32l" => (parent.new_i32(Endian::Little).this_mut(), parent),
                    "i32b" => (parent.new_i32(Endian::Big).this_mut(), parent),
                    "i32n" => (parent.new_i32(Endian::Native).this_mut(), parent),
                    "i64" | "i64l" => (parent.new_i64(Endian::Little).this_mut(), parent),
                    "i64b" => (parent.new_i64(Endian::Big).this_mut(), parent),
                    "i64n" => (parent.new_i64(Endian::Native).this_mut(), parent),
                    "i128" | "i128l" => (parent.new_i128(Endian::Little).this_mut(), parent),
                    "i128b" => (parent.new_i128(Endian::Big).this_mut(), parent),
                    "i128n" => (parent.new_i128(Endian::Native).this_mut(), parent),
                    "u8" => {
                        let t = unsafe {
                            &mut *(parent.new_u8() as *mut Type<'a>)
                        };
                        (t, parent)
                    }
                    "u16" | "u16l" => (parent.new_u16(Endian::Little).this_mut(), parent),
                    "u16b" => (parent.new_u16(Endian::Big).this_mut(), parent),
                    "u16n" => (parent.new_u16(Endian::Native).this_mut(), parent),
                    "u32" | "u32l" => (parent.new_u32(Endian::Little).this_mut(), parent),
                    "u32b" => (parent.new_u32(Endian::Big).this_mut(), parent),
                    "u32n" => (parent.new_u32(Endian::Native).this_mut(), parent),
                    "u64" | "u64l" => (parent.new_u64(Endian::Little).this_mut(), parent),
                    "u64b" => (parent.new_u64(Endian::Big).this_mut(), parent),
                    "u64n" => (parent.new_u64(Endian::Native).this_mut(), parent),
                    "u128" | "u128l" => (parent.new_u128(Endian::Little).this_mut(), parent),
                    "u128b" => (parent.new_u128(Endian::Big).this_mut(), parent),
                    "u128n" => (parent.new_u128(Endian::Native).this_mut(), parent),
                    "f32" | "f32l" => (parent.new_f32(Endian::Little).this_mut(), parent),
                    "f32b" => (parent.new_f32(Endian::Big).this_mut(), parent),
                    "f32n" => (parent.new_f32(Endian::Native).this_mut(), parent),
                    "f64" | "f64l" => (parent.new_f64(Endian::Little).this_mut(), parent),
                    "f64b" => (parent.new_f64(Endian::Big).this_mut(), parent),
                    "f64n" => (parent.new_f64(Endian::Native).this_mut(), parent),

                    // "map" | "flat_map" => {}
                    // "set" | "flat_set" => {}
                    //
                    // "struct" => {
                    //     // consts cannot inline declare a struct
                    //     if parent.is_const() {
                    //
                    //     }
                    //     // inline declared struct
                    // }
                    // "union" => {}
                    // "variant" => {}
                    "enum" => {
                        if parent.is_const() {
                            return Err(anyhow::Error::msg(ParseError::InvalidConst {
                                token: self.token_span(),
                                message: Some("const cannot inline declare an enum".to_owned()),
                            }));
                        }
                        self.next_essential()?;
                        let (t, parent) = self.parse_enum(parent, true)?;
                        (t.this_mut(), parent)
                    }

                    _ => {
                        if name.starts_with("string") {
                            if name == "string" {
                                (parent.new_string().this_mut(), parent)
                            } else {
                                let size_str = &name[6..];
                                match usize::from_str(size_str) {
                                    Ok(size) => {
                                        match self.peek() {
                                            Some('.') => match self.peek() {
                                                Some('.') => {
                                                    self.expect_next_token(Token::DotDot)?;
                                                    (parent.new_string_inline_plus(size).this_mut(), parent)
                                                }
                                                Some(_) | None => (parent.new_string_inline(size).this_mut(), parent)
                                            }
                                            Some(_) | None => (parent.new_string_inline(size).this_mut(), parent)
                                        }
                                    }
                                    Err(_) => return Err(anyhow::Error::msg(ParseError::BadNumeral {
                                        token: self.token_span(),
                                        reason: "expected a string length".to_owned(),
                                    }))
                                }
                            }
                        } else {
                            // user-defined
                            (parent.new_unknown(Some(name)).this_mut(), parent)
                        }
                    }
                }
            }
            _ => {
                return Err(anyhow::Error::msg(ParseError::ExpectedOneOf {
                    one_of: vec![
                        Token::QuestionMark,
                        Token::Star,
                        Token::BracketOpen,
                        Token::Word,
                    ],
                    found: self.token_span(),
                }));
            }
        };

        if is_field {
            loop {
                match self.expect_next()? {
                    Token::Whitespace => {}
                    Token::NewLine => {
                        self.next_essential()?;
                        return Ok(new_type);
                    }
                    Token::LineComment => {
                        let line_comment = self.comments.take();
                        parent.set_inline_comment(line_comment);
                        self.next_essential()?;
                        return Ok(new_type);
                    }
                    Token::BlockComment => {
                        self.next_essential()?;
                        return Ok(new_type);
                    }
                    Token::Comma | Token::Semicolon => {
                        self.next_essential()?;
                        return Ok(new_type);
                    }
                    Token::Equals => {
                        if !new_type.can_default() {
                            return Err(anyhow::Error::msg(ParseError::VariantFieldDefault {
                                token: self.token_position(),
                            }));
                        }
                        self.next_essential()?;
                    }
                    Token::At | Token::Word | Token::Numeral => {
                        return Ok(new_type);
                    }
                    _ => {}
                }
            }
        }

        Ok(new_type)
    }

    fn parse_value(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn consume_comments(&mut self) -> anyhow::Result<Token> {
        let mut token = self.token;
        loop {
            match token {
                Token::NewLine => {}
                Token::LineComment => {}
                Token::BlockComment => {}
                _ => return Ok(token),
            }
            token = self.expect_next()?;
        }
    }

    fn expect_positive_int(&mut self) -> anyhow::Result<usize> {
        self.expect_token(Token::Numeral)?;
        match &self.numeral {
            None => Ok(0),
            Some(v) => match v {
                Numeral::Float { text, value } => {
                    Err(anyhow::Error::msg(ParseError::InvalidFieldNumber {
                        reason: "cannot be a float",
                        float: Some(*value),
                        int: None,
                    }))
                }
                Numeral::Int { text, value } => {
                    if *value < 1 {
                        Err(anyhow::Error::msg(ParseError::InvalidFieldNumber {
                            reason: "cannot be less than 1",
                            float: None,
                            int: Some(*value),
                        }))
                    } else {
                        Ok(*value as usize)
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
                        Ok(*value as usize)
                    }
                }
            },
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
        let mut m = Module::new("".to_owned());

        let s = m.new_struct();

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
        let file = std::env::current_dir().unwrap().to_str().unwrap();
        println!("{}", std::env::current_dir().unwrap().to_str().unwrap());
        match std::fs::read_to_string(Path::new("src/testdata/s.wasp")) {
            Ok(s) => match Parser::parse(&s) {
                Ok(_module) => {
                    println!("done!");
                }
                Err(reason) => {
                    let err = reason.to_string();
                    println!("{}", reason.to_string());
                }
            },
            Err(reason) => {
                println!("{}", reason.to_string());
            }
        }
    }
}
