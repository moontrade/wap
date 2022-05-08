use std::{mem, ptr};
use std::borrow::BorrowMut;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ops::{Deref, DerefMut};
use std::ptr::{null, null_mut};
use std::rc::{Rc, Weak};

use anyhow::Error;

use crate::parser::ParseError;

#[derive(Clone)]
pub enum Kind<'a> {
    Module(&'a Module<'a>),
    Unknown,
    Bool,
    I8,
    U8,
    I16(Number<'a>),
    U16(Number<'a>),
    I32(Number<'a>),
    U32(Number<'a>),
    I64(Number<'a>),
    U64(Number<'a>),
    I128(Number<'a>),
    U128(Number<'a>),
    F32(Number<'a>),
    F64(Number<'a>),
    F128(Number<'a>),
    // heap allocated string
    String(Str<'a>),
    // string embedded inline
    StringInline(Str<'a>),
    // string embedded inline with a heap allocated spill-over
    StringInlinePlus(Str<'a>),
    Optional(Optional<'a>),
    Pointer(Pointer<'a>),
    Enum(Enum<'a>),
    EnumOption(EnumOption<'a>),
    Struct(Struct<'a>),
    StructField(StructField<'a>),
    Union(Union<'a>),
    UnionField(UnionField<'a>),
    Variant(Variant<'a>),
    VariantOption(VariantOption<'a>),
    Array(Vector<'a>),
    Vector(Vector<'a>),
    // robin-hood flat map
    FlatMap(Map<'a>),
    NodeMap(Map<'a>),
    FlatSet(Map<'a>),
    NodeSet(Map<'a>),
    // ART radix tree
    OrderedMap(Map<'a>),
    OrderedSet(Map<'a>),
    // B+Tree
    TreeMap(Map<'a>),
    TreeSet(Map<'a>),
    Const(Const<'a>),
    Alias(Alias<'a>),
    Pad(usize),
}

impl<'a> Kind<'a> {
    pub fn display_name(&self) -> &'static str {
        match self {
            Kind::Module(_) => "Module",
            Kind::Unknown => "Unknown",
            Kind::Bool => "bool",
            Kind::I8 => "i8",
            Kind::U8 => "u8",
            Kind::I16(_) => "i16",
            Kind::U16(_) => "u16",
            Kind::I32(_) => "i32",
            Kind::U32(_) => "u32",
            Kind::I64(_) => "i64",
            Kind::U64(_) => "u64",
            Kind::I128(_) => "i128",
            Kind::U128(_) => "u128",
            Kind::F32(_) => "f32",
            Kind::F64(_) => "f64",
            Kind::F128(_) => "f128",
            Kind::String(_) => "string",
            Kind::StringInline(_) => "string_inline",
            Kind::StringInlinePlus(_) => "string_inline_plus",
            Kind::Optional(_) => "optional",
            Kind::Pointer(_) => "pointer",
            Kind::Enum(_) => "enum",
            Kind::EnumOption(_) => "enum_option",
            Kind::Struct(_) => "struct",
            Kind::StructField(_) => "struct_field",
            Kind::Union(_) => "union",
            Kind::UnionField(_) => "union_field",
            Kind::Variant(_) => "variant",
            Kind::VariantOption(_) => "variant_option",
            Kind::Array(_) => "array",
            Kind::Vector(_) => "vector",
            Kind::FlatMap(_) => "flat_map",
            Kind::NodeMap(_) => "node_map",
            Kind::FlatSet(_) => "flat_set",
            Kind::NodeSet(_) => "node_set",
            Kind::OrderedMap(_) => "ordered_map",
            Kind::OrderedSet(_) => "ordered_set",
            Kind::TreeMap(_) => "tree_map",
            Kind::TreeSet(_) => "tree_set",
            Kind::Const(_) => "const",
            Kind::Alias(_) => "alias",
            Kind::Pad(_) => "pad ..",
            _ => ""
        }
    }

    pub fn name(&self) -> &'a str {
        match self {
            _ => "",
        }
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Position {
    pub line: usize,
    pub col: usize,
    pub index: usize,
}

impl Position {
    pub fn new(line: usize, col: usize, index: usize) -> Self {
        Self { line, col, index }
    }
}

pub struct Comments<'a> {
    orphans: Vec<Comment<'a>>,
    active: Option<Comment<'a>>,
}

impl<'a> Comments<'a> {
    pub fn new() -> Self {
        Self { orphans: Vec::new(), active: None }
    }

    pub fn push(&mut self, multiline: bool, begin: Position, end: Position, text: &'a str) {
        match self.active.take() {
            Some(mut c) => {
                match c {
                    Comment::Single(line) => {
                        if multiline {
                            self.orphans.push(Comment::Single(line));
                            self.active = Some(Comment::Multi(vec![CommentLine::new(begin, end, text)]));
                        } else if line.begin.line == begin.line - 1 {
                            self.active = Some(Comment::Group(vec![line, CommentLine::new(begin, end, text)]));
                        } else {
                            self.orphans.push(Comment::Single(line));
                            self.active = Some(Comment::Single(CommentLine::new(begin, end, text)));
                        }
                    }
                    Comment::Group(mut lines) => {
                        if multiline {
                            self.orphans.push(Comment::Group(lines));
                            self.active = Some(Comment::Multi(vec![CommentLine::new(begin, end, text)]));
                        } else if lines.len() > 0 && lines.last().unwrap().begin.line == begin.line - 1 {
                            lines.push(CommentLine::new(begin, end, text));
                            self.active = Some(Comment::Group(lines));
                        } else {
                            self.orphans.push(Comment::Group(lines));
                            self.active = Some(Comment::Single(CommentLine::new(begin, end, text)));
                        }
                    }
                    Comment::Multi(mut lines) => {
                        if multiline {
                            if lines.len() > 0 && lines.last().unwrap().begin.line == begin.line - 1 {
                                lines.push(CommentLine::new(begin, end, text));
                                self.active = Some(Comment::Multi(lines));
                            } else {
                                self.orphans.push(Comment::Multi(lines));
                                self.active = Some(Comment::Multi(vec![CommentLine::new(begin, end, text)]));
                            }
                        } else {
                            self.orphans.push(Comment::Multi(lines));
                            self.active = Some(Comment::Single(CommentLine::new(begin, end, text)));
                        }
                    }
                }
            }
            None => {
                if multiline {
                    self.active = Some(Comment::Multi(vec![CommentLine::new(begin, end, text)]));
                } else {
                    self.active = Some(Comment::Single(CommentLine::new(begin, end, text)));
                }
            }
        }
    }

    pub fn take(&mut self) -> Option<Comment<'a>> {
        self.active.take()
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum Comment<'a> {
    Single(CommentLine<'a>),
    Group(Vec<CommentLine<'a>>),
    Multi(Vec<CommentLine<'a>>),
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct CommentLine<'a> {
    pub begin: Position,
    pub end: Position,
    pub value: &'a str,
}

impl<'a> CommentLine<'a> {
    pub fn new(begin: Position, end: Position, value: &'a str) -> Self {
        Self { begin, end, value }
    }
}

pub struct Mut<'a, T>(*mut T, PhantomData<(&'a ())>);

impl<'a, T> Mut<'a, T> {
    pub fn new(r: *mut T) -> Self {
        Self(r, PhantomData)
    }
}

impl<'a, T> Deref for Mut<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.0 }
    }
}

impl<'a, T> DerefMut for Mut<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.0 }
    }
}

impl<'a, T> Clone for Mut<'a, T> {
    fn clone(&self) -> Self {
        Self(self.0, PhantomData)
    }
}

pub struct Module<'a> {
    pub full: String,
    pub name: String,
    root: Type<'a>,
    // Type storage
    types: Vec<Type<'a>>,
    inner: Vec<*const Type<'a>>,
    // Named type index
    // types_map: BTreeMap<&'a str, &'a Type<'a>>,
    // Value storage.
    values: Vec<Value<'a>>,
    _marker: PhantomData<(&'a ())>,
}

impl<'a> Module<'a> {
    pub(crate) fn new() -> Box<Self> {
        let mut m = Box::new(Self {
            full: String::from(EMPTY),
            name: String::from(EMPTY),
            root: unsafe { MaybeUninit::uninit().assume_init() },
            types: Vec::new(),
            inner: Vec::new(),
            // types_map: BTreeMap::new(),
            values: Vec::new(),
            _marker: PhantomData,
        });
        let r = m.as_mut();
        r.root = Type::new(
            unsafe {&*(r as *mut Module<'a> as *const Module<'a>)},
            None,
            None);
        r.root.kind = Kind::Module(unsafe {&*(r as *mut Module<'a> as *const Module<'a>)});
        m
    }

    pub fn types_iter(&self) -> impl Iterator<Item=&Type<'a>> {
        self.types.iter()
    }

    pub fn root(&mut self) -> &Type<'a> {
        &self.root
    }

    pub fn root_mut(&mut self) -> &mut Type<'a> {
        &mut self.root
    }

    pub fn new_enum(&mut self) -> &mut Enum<'a> {
        let e = (unsafe {&mut *(self as *mut Self) }).root_mut().new_enum();
        self.inner.push(e.owner());
        e
    }

    pub fn new_struct(&mut self) -> &mut Struct<'a> {
        let s = (unsafe {&mut *(self as *mut Self) }).root_mut().new_struct();
        self.inner.push(s.owner());
        s
    }

    pub fn new_union(&mut self) -> &mut Union<'a> {
        let u = (unsafe {&mut *(self as *mut Self) }).root_mut().new_union();
        self.inner.push(u.owner());
        u
    }
}

#[derive(Clone)]
pub struct Import<'a> {
    module: &'a Module<'a>,
    alias: String,
}

pub struct Type<'a> {
    pub name: Option<String>,
    pub kind: Kind<'a>,
    module: &'a Module<'a>,
    parent: Option<&'a Type<'a>>,
    value: Option<*const Value<'a>>,
}

const EMPTY: &'static str = "";

impl<'a> Type<'a> {
    fn new(
        module: &'a Module<'a>,
        parent: Option<&'a Type<'a>>,
        name: Option<String>,
    ) -> Self {
        Self {
            name,
            kind: Kind::Unknown,
            module,
            parent,
            value: None,
        }
    }

    pub fn inner_mut(&mut self) -> Option<&mut Option<Vec<&'a Type<'a>>>> {
        match &mut self.kind {
            Kind::Struct(s) => Some(&mut s.inner),
            _ => None
        }
    }

    pub fn module(&self) -> &Module<'a> {
        unsafe { &*self.module }
    }

    pub fn module_mut(&mut self) -> &mut Module<'a> {
        unsafe { &mut *(self.module as *const Module<'a> as *mut Module<'a>) }
    }

    fn parent(&self) -> Option<&Type<'a>> {
        match self.parent {
            Some(p) => Some(p),
            None => None
        }
    }

    fn parent_mut(&'a mut self) -> Option<&'a mut Type<'a>> {
        match self.parent {
            Some(p) => Some(unsafe { &mut *(p as *const Type<'a> as *mut Type<'a>) }),
            None => None
        }
    }

    pub fn name(&self) -> &str {
        match &self.name {
            Some(ref s) => s.as_str(),
            None => &EMPTY,
        }
    }

    // pub fn new_enum<R>(&'a mut self,
    //                    f: impl FnOnce(
    //                        &'a mut Enum<'a>,
    //                    ) -> anyhow::Result<R>) -> anyhow::Result<R> {
    //     let parent = unsafe { self as *mut Self };
    //     self.module_mut().new_enum_with_parent(parent, f)
    // }

    fn push(&mut self,
            f: impl FnOnce(&'a Type<'a>) -> Kind<'a>,
    ) -> &mut Type<'a> {
        (unsafe { &mut *(self.module as *const Module<'a> as *mut Module<'a>) }).
            types.push(
            Type::new(self.module, Some(unsafe { &*(self as *const Type<'a>) }), None)
        );
        let t = (unsafe { &mut *(self.module as *const Module<'a> as *mut Module<'a>) }).types.last_mut().unwrap();
        t.kind = f(unsafe { &*(t as *const Type<'a>) });
        t
    }

    // fn new_enum<R>(
    //     &mut self,
    //     f: impl FnOnce(&mut Enum<'a>) -> anyhow::Result<R>,
    // ) -> anyhow::Result<R> {
    //     match self.push(|t| Kind::Enum(Enum::new(t))).kind {
    //         Kind::Enum(ref mut e) => f(e),
    //         _ => panic!("unreachable"),
    //     }
    // }

    fn new_enum(&mut self) -> &mut Enum<'a> {
        match self.push(|t| Kind::Enum(Enum::new(t))).kind {
            Kind::Enum(ref mut e) => e,
            _ => panic!("unreachable"),
        }
    }

    fn new_struct(&mut self) -> &mut Struct<'a> {
        match self.push(|t| Kind::Struct(Struct::new(t))).kind {
            Kind::Struct(ref mut s) => s,
            _ => panic!("unreachable"),
        }
    }

    fn new_union(&mut self) -> &mut Union<'a> {
        match self.push(|t| Kind::Union(Union::new(t))).kind {
            Kind::Union(ref mut u) => u,
            _ => panic!("unreachable"),
        }
    }

    fn new_variant(&mut self) -> &mut Variant<'a> {
        match self.push(|t| Kind::Variant(Variant::new(t))).kind {
            Kind::Variant(ref mut v) => v,
            _ => panic!("unreachable"),
        }
    }

    // pub fn new_union<R>(&'a mut self,
    //                     f: impl FnOnce(
    //                         &'a mut Union<'a>,
    //                     ) -> anyhow::Result<R>) -> anyhow::Result<R> {
    //     let parent = unsafe { self as *mut Self };
    //     self.module_mut().new_union_with_parent(parent, f)
    // }
    //
    // pub fn new_variant<R>(&'a mut self,
    //                       f: impl FnOnce(
    //                           &'a mut Variant<'a>,
    //                       ) -> anyhow::Result<R>) -> anyhow::Result<R> {
    //     let parent = unsafe { self as *mut Self };
    //     self.module_mut().new_variant_with_parent(parent, f)
    // }

    // pub fn new_string<R>(&'a mut self,
    //                      inline_size: usize,
    //                      max_size: usize, ) -> &'a mut Str<'a> {
    //     let parent = unsafe { self as *mut Self };
    //     let t = self.module_mut().push_with_parent(parent, Kind::String(Str::new(inline_size, max_size)));
    //     match t.kind {
    //         Kind::String(ref mut s) => s,
    //         _ => panic!("unreachable")
    //     }
    // }

    // pub fn new_unknown(&mut self) -> &mut Type<'a> {
    //     self.module_mut().push_with_parent0(unsafe { &*self }, |t| Kind::Unknown)
    // }


    pub fn new_bool(&mut self) -> &mut Type<'a> {
        self.push(|_| Kind::Bool)
    }
    pub fn new_i8(&mut self) -> &mut Type<'a> {
        self.push(|_| Kind::I8)
    }
    pub fn new_u8(&mut self) -> &mut Type<'a> {
        self.push(|_| Kind::U8)
    }
    pub fn new_u16(&'a mut self, endian: Endian) -> &'a mut Number<'a> {
        match self.push(|t| Kind::U16(Number::new(endian))).kind {
            Kind::U16(ref mut n) => n,
            _ => panic!("unreachable"),
        }
    }
    // pub fn new_u32(&'a mut self, endian: Endian) -> &'a mut Number<'a> {
    //     self.new_number(Kind::U32(Number::new(endian)))
    // }
    // pub fn new_u64(&'a mut self, endian: Endian) -> &'a mut Number<'a> {
    //     self.new_number(Kind::U64(Number::new(endian)))
    // }
    // pub fn new_u128(&'a mut self, endian: Endian) -> &'a mut Number<'a> {
    //     self.new_number(Kind::U128(Number::new(endian)))
    // }
    // pub fn new_i8(&'a mut self) -> &'a mut Type<'a> {
    //     self.module_mut().push(Kind::I8)
    // }
    // pub fn new_i16(&'a mut self, endian: Endian) -> &'a mut Number<'a> {
    //     let parent = unsafe { self as *mut Self };
    //     self.new_number(Kind::I16(Number::new(endian)))
    // }
    // pub fn new_i32(&'a mut self, endian: Endian) -> &'a mut Number<'a> {
    //     let parent = unsafe { self as *mut Self };
    //     self.new_number(Kind::I32(Number::new(endian)))
    // }
    // pub fn new_i64(&'a mut self, endian: Endian) -> &'a mut Number<'a> {
    //     let parent = unsafe { self as *mut Self };
    //     self.new_number(Kind::I64(Number::new(endian)))
    // }
    // pub fn new_i128(&'a mut self, endian: Endian) -> &'a mut Number<'a> {
    //     self.new_number(Kind::I128(Number::new(endian)))
    // }
    // pub fn new_f32(&'a mut self, endian: Endian) -> &'a mut Number<'a> {
    //     self.new_number(Kind::F32(Number::new(endian)))
    // }
    // pub fn new_f64(&'a mut self, endian: Endian) -> &'a mut Number<'a> {
    //     self.new_number(Kind::F64(Number::new(endian)))
    // }
    //
}

pub trait KindData<'a> {
    // fn new(owner: &'a Type<'a>) -> Self;

    fn owner_ptr(&self) -> *const Type<'a>;

    fn module(&self) -> &Module<'a> {
        self.owner().module()
    }

    fn module_mut(&mut self) -> &'a mut Module<'a> {
        (unsafe { &mut *(self.owner_ptr() as *mut Type<'a>) }).module_mut()
    }

    fn owner(&self) -> &'a Type<'a> {
        unsafe { &*self.owner_ptr() }
    }

    fn owner_mut(&mut self) -> &'a mut Type<'a> {
        unsafe { &mut *(self.owner_ptr() as *mut Type<'a>) }
    }

    fn name(&self) -> &'a str {
        match &self.owner().name {
            None => EMPTY,
            Some(v) => v.as_str()
        }
    }

    fn set_name(&mut self, name: String) {
        self.owner_mut().name = Some(name);
    }
}

#[derive(Clone)]
pub struct Const<'a> {
    owner: *const Type<'a>,
    value: Value<'a>,
}

#[derive(Clone)]
pub struct Alias<'a> {
    owner: &'a Type<'a>,
    value: &'a Type<'a>,
}

#[derive(Clone)]
pub struct Optional<'a> {
    owner: &'a Type<'a>,
    inner: &'a Type<'a>,
}

#[derive(Clone)]
pub struct Pointer<'a> {
    owner: *const Type<'a>,
    inner: *const Type<'a>,
}

#[derive(Copy, Clone, PartialEq)]
pub enum Endian {
    Little = 0,
    Big = 1,
    Native = 2,
}

#[derive(Clone, PartialEq)]
pub struct Number<'a> {
    owner: *const Type<'a>,
    endian: Endian,
}

impl<'a> Number<'a> {
    fn new(endian: Endian) -> Self {
        Self {
            owner: null(),
            endian,
        }
    }
}

#[derive(Clone)]
pub struct Str<'a> {
    owner: &'a Type<'a>,
    inline_size: usize,
    max_size: usize,
}

impl<'a> Str<'a> {
    fn new(owner: &'a Type<'a>, inline_size: usize, max_size: usize) -> Self {
        Self {
            owner,
            inline_size,
            max_size,
        }
    }

    pub fn is_inline(&self) -> bool {
        self.inline_size > 0
    }
}

#[derive(Clone)]
pub struct Struct<'a> {
    owner: &'a Type<'a>,
    align: u16,
    pack: u16,
    size: u32,
    fields: Vec<&'a StructField<'a>>,
    inner: Option<Vec<&'a Type<'a>>>,
}

impl<'a> Struct<'a> {
    fn new(owner: &'a Type<'a>) -> Self {
        Self {
            owner,
            align: 0,
            pack: 0,
            size: 0,
            fields: Vec::new(),
            inner: None,
        }
    }
}

impl<'a> KindData<'a> for Struct<'a> {
    fn owner_ptr(&self) -> *const Type<'a> {
        self.owner
    }
}

#[derive(Clone)]
pub struct StructField<'a> {
    owner: &'a Type<'a>,
}

#[derive(Clone)]
pub struct Union<'a> {
    owner: &'a Type<'a>,
    fields: Vec<*const UnionField<'a>>,
    inner: Option<Vec<&'a Type<'a>>>,
}

impl<'a> Union<'a> {
    fn new(owner: &'a Type<'a>) -> Self {
        Self {
            owner,
            fields: Vec::new(),
            inner: None,
        }
    }
}

impl<'a> KindData<'a> for Union<'a> {
    fn owner_ptr(&self) -> *const Type<'a> {
        self.owner
    }
}

#[derive(Clone)]
pub struct UnionField<'a> {
    parent: *const Type<'a>,
    owner: *const Type<'a>,
}

#[derive(Clone)]
pub struct Variant<'a> {
    owner: &'a Type<'a>,
    inner: Option<Vec<&'a Type<'a>>>,
}

impl<'a> Variant<'a> {
    fn new(owner: &'a Type<'a>) -> Self {
        Self {
            owner,
            inner: None,
        }
    }
}

impl<'a> KindData<'a> for Variant<'a> {
    fn owner_ptr(&self) -> *const Type<'a> {
        self.owner
    }
}

#[derive(Clone, PartialEq)]
pub struct VariantOption<'a> {
    parent: *const Variant<'a>,
    owner: *const Type<'a>,
}

#[derive(Clone, PartialEq)]
pub struct Enum<'a> {
    owner: *const Type<'a>,
}

impl<'a> Enum<'a> {
    fn new(owner: &'a Type<'a>) -> Self {
        Self { owner }
    }
}

impl<'a> KindData<'a> for Enum<'a> {
    fn owner_ptr(&self) -> *const Type<'a> {
        self.owner
    }
}

#[derive(Clone)]
pub struct EnumOption<'a> {
    owner: *const Type<'a>,
    value: *const Value<'a>,
}

#[derive(Clone)]
pub struct Vector<'a> {
    owner: &'a Type<'a>,
    element: *const Type<'a>,
}

impl<'a> Vector<'a> {
    fn new(owner: &'a Type<'a>) -> Self {
        Self {
            owner,
            element: null(),
        }
    }
}

impl<'a> KindData<'a> for Vector<'a> {
    fn owner_ptr(&self) -> *const Type<'a> {
        self.owner
    }
}

#[derive(Clone)]
pub struct Map<'a> {
    owner: *const Type<'a>,
    key: *const Type<'a>,
    value: *const Type<'a>,
}

impl<'a> Map<'a> {
    fn new(owner: &'a Type<'a>) -> Self {
        Self {
            owner,
            key: null(),
            value: null(),
        }
    }
}

impl<'a> KindData<'a> for Map<'a> {
    fn owner_ptr(&self) -> *const Type<'a> {
        self.owner
    }
}


#[derive(Clone, PartialEq)]
pub enum Value<'a> {
    Nil,
    Bool(bool),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    I128(i128),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    U128(u128),
    F32(f32),
    F64(f64),
    String(String),
    Struct(Object<'a>),
    Union(Object<'a>),
    Variant(Object<'a>),
    Enum(*const EnumOption<'a>),
    Const(*const Const<'a>),
    Object(Object<'a>),
}

#[derive(Clone, PartialEq)]
pub struct Object<'a> {
    pub props: Vec<ObjectProperty<'a>>,
}

impl<'a> Object<'a> {
    pub fn new() -> Self {
        Self {
            props: Vec::new(),
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct ObjectProperty<'a> {
    pub name: Option<String>,
    pub value: Value<'a>,
}

impl<'a> ObjectProperty<'a> {
    pub fn new(name: Option<String>, value: Value<'a>) -> Self {
        Self { name, value }
    }
}

#[cfg(test)]
mod tests {
    use std::io::BufReader;

    use super::*;

    #[test]
    fn test_type() {
        let mut m = Module::new();

        let s = m.new_struct();
        s.set_name(String::from("Order"));
        s.module_mut();

        // let (mut t, o) = Type::new_struct(Some(String::from("Order")));
        //
        // Type::new_struct_fn(Some(String::from("Order")), |t, s| {
        //
        // });
        //
        // println!("{}", t.name());
        println!("done");
    }
}