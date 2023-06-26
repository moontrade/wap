use std::ops::Deref;
use std::rc::Rc;
use std::cell::RefCell;
use std::cell::RefMut;

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Position {
    pub line: i32,
    pub col: i32,
    pub index: i32,
}

impl Position {
    pub fn new(line: i32, col: i32, index: i32) -> Self {
        Self { line, col, index }
    }
}

pub struct Module {
    children: Vec<Kind>,
}

impl Module {
    pub fn new() -> Self {
        Self {
            children: vec![],
        }
    }

    pub fn new_struct(&mut self, mut f: impl FnMut(Kind, RefMut<Struct>)) {
        let mut s = Rc::new(RefCell::new(Struct::new(None)));
        // let mut ss = unsafe { &mut *((s.deref() as *const Struct) as *mut Struct) };
        f(Kind::Struct(Rc::clone(&s)), s.borrow_mut());
        self.children.push(Kind::Struct(Rc::clone(&s)));
    }
}

pub enum ParentKind {
    Module(Rc<RefCell<Module>>),
    Struct(Rc<RefCell<Struct>>),
}

pub enum Kind {
    Module(Rc<RefCell<Module>>),
    I32(Num),
    Struct(Rc<RefCell<Struct>>),
}

pub struct Num {
    endian: u8,
}

pub struct Struct {
    parent: Option<Kind>,
    this: Option<Kind>,
    name: Option<String>,
    fields: Vec<Field>,
    position: Position,
    children: Option<Vec<Kind>>,
}

impl Struct {
    pub fn new(parent: Option<Kind>) -> Self {
        Self {
            parent,
            this: None,
            name: None,
            fields: vec![],
            position: Position::new(0, 0, 0),
            children: None,
        }
    }

    pub fn name(&self) -> &str {
        match &self.name {
            Some(s) => s.as_str(),
            None => ""
        }
    }

    pub fn set_name(&mut self, name: Option<String>) -> &mut Self {
        self.name = name;
        self
    }

    fn push(&mut self, kind: Kind) {
        match self.children {
            Some(ref mut children) => children.push(kind),
            None => self.children = Some(vec![kind])
        }
    }

    pub fn new_struct(&mut self, mut f: impl FnMut(Kind, RefMut<Struct>)) {
        let mut s = Rc::new(RefCell::new(Struct::new(None)));
        f(Kind::Struct(Rc::clone(&s)), s.borrow_mut());
        self.push(Kind::Struct(Rc::clone(&s)));
    }
}

pub struct Field {
    name: String,
    position: Position,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type() {
        let mut m = Module::new();
        m.new_struct(|kind, mut s| {
            s.set_name(Some("Order".to_owned()));
        });

        println!("done");
    }
}