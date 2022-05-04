use std::mem::MaybeUninit;
use std::ops::Deref;
use std::str::CharIndices;

use crate::model::*;

use super::model;

pub struct Parser<'a> {
    contents: &'a String,
    iter: CharIndices<'a>,
    line: usize,
    byte_index: usize,
    mark: usize,
    char_index: usize,
    module: Option<Module<'a>>,
    prev: TokenKind<'a>,
    current: TokenKind<'a>,
    next: TokenKind<'a>
}

pub struct ParseError {}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Token<'a> {
    pub kind: TokenKind<'a>,
    pub start: Position,
    pub end: Position,
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum TokenKind<'a> {
    None,
    EOF,
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
    CurlyBraceOpen,
    CurlyBraceClose,
    At,
    DoubleQuote,
    SingleQuote,
    LessThan,
    GreaterThan,
    BracketOpen,
    BracketClose,
    Carat,
    Ampersand,
    Equals,
    Comment(Comment<'a>),
    CommentMulti(Comment<'a>),
    Name(&'a str),
}

impl<'a> Parser<'a> {
    pub fn new(contents: &'a String) -> Self {
        Self {
            contents,
            iter: contents.char_indices(),
            line: 1,
            byte_index: 0,
            mark: 0,
            char_index: 0,
            module: None,
            prev: TokenKind::None,
            current: TokenKind::None,
            next: TokenKind::None,
        }
    }

    #[inline(always)]
    fn advance(&mut self) -> Option<char> {
        match self.iter.next() {
            None => None,
            Some((s, c)) => {
                self.byte_index += s;
                self.char_index += 1;
                Some(c)
            }
        }
    }

    fn next(&mut self) -> TokenKind {
        self.prev = self.current;
        if self.next != TokenKind::None {
            let r = self.next;
            self.next = TokenKind::None;
            return r;
        }
        self.mark = self.byte_index;
        loop {
            match self.advance() {
                None => {
                    self.current = TokenKind::EOF;
                    break;
                },
                Some(ch) => {
                    match ch {
                        '\n' => {
                            let current = self.char_index;
                            let slice = &self.contents[self.mark..self.byte_index];
                            if slice.len() > 0 {
                                self.next = TokenKind::NewLine;
                                return TokenKind::Name(slice);
                            }
                        }
                        ' ' => {
                            // re
                        }
                        '{' => {}
                        '/' => {
                            match self.advance() {
                                None => return TokenKind::EOF,
                                Some(ch) => {
                                    match ch {
                                        '/' => {},
                                        '*' => {
                                            // Multi line
                                        }
                                        _ => {
                                            self.current = TokenKind::ForwardSlash;
                                            return self.current;
                                        }
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        self.current
    }
}

#[cfg(test)]
mod tests {
    use crate::model::*;

    use super::*;

    #[test]
    fn test_type() {
        let mut m = Module::new(String::from(""), String::from("wap::model"), String::from("model"));

        // m.new_struct(move |mut m, t, s| {
        //     t.name = Some(String::from("Order"));
        //     Ok(())
        // });
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
        let s = String::new();
        let mut p = Parser::new(&s);
    }
}