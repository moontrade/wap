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
    Empty,
    Module(&'a Module<'a>),
    Unknown(Unknown<'a>),
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
    Field(Field<'a>),
    Union(Struct<'a>),
    Variant(Struct<'a>),
    Array(Array<'a>),
    ArrayVector(ArrayVector<'a>),
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
            Kind::Empty => "<EMPTY>",
            Kind::Module(_) => "Module",
            Kind::Unknown(_) => "Unknown",
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
            Kind::StringInline(_) => "string{0}",
            Kind::StringInlinePlus(_) => "string{0}..",
            Kind::Optional(_) => "optional",
            Kind::Pointer(_) => "pointer",
            Kind::Enum(_) => "enum",
            Kind::EnumOption(_) => "enum_option",
            Kind::Struct(_) => "struct",
            Kind::Field(_) => "field",
            Kind::Union(_) => "union",
            Kind::Variant(_) => "variant",
            Kind::Array(_) => "array",
            Kind::ArrayVector(_) => "array..",
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
    pub full_name: String,
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
            full_name: String::from(EMPTY),
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
            unsafe { &*(r as *mut Module<'a> as *const Module<'a>) },
            None);
        r.root.kind = Kind::Module(unsafe { &*(r as *mut Module<'a> as *const Module<'a>) });
        m
    }

    pub fn set_full_name(&mut self, value: String) {
        self.full_name = value;
    }

    pub fn set_name(&mut self, value: String) {
        self.name = value;
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

    pub fn new_alias(&mut self, name: &str, of: Option<&'a Type<'a>>) -> &mut Alias<'a> {
        let a = (unsafe { &mut *(self as *mut Self) }).root_mut().new_alias(name.to_owned(), of);
        self.inner.push(a.this());
        a
    }

    pub fn new_enum(&mut self) -> &mut Enum<'a> {
        let e = (unsafe { &mut *(self as *mut Self) }).root_mut().new_enum();
        self.inner.push(e.this());
        e
    }

    pub fn new_struct(&mut self) -> &mut Struct<'a> {
        let s = (unsafe { &mut *(self as *mut Self) }).root_mut().new_struct(StructKind::Struct);
        self.inner.push(s.this());
        s
    }

    pub fn new_union(&mut self) -> &mut Struct<'a> {
        let u = (unsafe { &mut *(self as *mut Self) }).root_mut().new_struct(StructKind::Union);
        self.inner.push(u.this());
        u
    }

    pub fn new_variant(&mut self) -> &mut Struct<'a> {
        let u = (unsafe { &mut *(self as *mut Self) }).root_mut().new_struct(StructKind::Variant);
        self.inner.push(u.this());
        u
    }
}

#[derive(Clone)]
pub struct Import<'a> {
    module: &'a Module<'a>,
    alias: String,
}

pub struct Type<'a> {
    // pub name: Option<String>,
    kind: Kind<'a>,
    module: &'a Module<'a>,
    parent: Option<&'a Type<'a>>,
    value: Option<*const Value<'a>>,
}

const EMPTY: &'static str = "";

impl<'a> Type<'a> {
    fn new(
        module: &'a Module<'a>,
        parent: Option<&'a Type<'a>>,
    ) -> Self {
        Self {
            // name,
            kind: Kind::Empty,
            module,
            parent,
            value: None,
        }
    }

    pub fn kind(&'a self) -> &'a Kind<'a> {
        &self.kind
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

    pub fn parent(&self) -> Option<&Type<'a>> {
        match self.parent {
            Some(p) => Some(p),
            None => None
        }
    }

    pub fn parent_mut(&'a mut self) -> Option<&'a mut Type<'a>> {
        match self.parent {
            Some(p) => Some(unsafe { &mut *(p as *const Type<'a> as *mut Type<'a>) }),
            None => None
        }
    }

    pub fn is_const(&self) -> bool {
        match self.kind {
            Kind::Const(_) => true,
            _ => false
        }
    }

    pub fn is_alias(&self) -> bool {
        match self.kind {
            Kind::Alias(_) => true,
            _ => false
        }
    }

    pub fn parent_is_const(&self) -> bool {
        match self.parent {
            Some(p) => p.is_const(),
            None => false
        }
    }

    pub fn parent_is_alias(&self) -> bool {
        match self.parent {
            Some(p) => p.is_alias(),
            None => false
        }
    }

    pub fn parent_is_field(&self) -> bool {
        match self.parent {
            Some(p) => p.is_field(),
            None => false
        }
    }

    pub fn can_default(&self) -> bool {
        match &self.parent {
            Some(p) => match &p.kind {
                Kind::Field(f) => f.parent.kind == StructKind::Struct,
                _ => false
            },
            None => false
        }
    }

    pub fn is_field(&self) -> bool {
        match self.kind {
            Kind::Field(_) => true,
            _ => false
        }
    }

    pub fn is_module(&self) -> bool {
        match self.kind {
            Kind::Module(_) => true,
            _ => false
        }
    }

    pub fn name(&self) -> &str {
        self.kind.name()
    }

    fn push(&mut self,
            f: impl FnOnce(&'a Type<'a>) -> Kind<'a>,
    ) -> &mut Type<'a> {
        (unsafe { &mut *(self.module as *const Module<'a> as *mut Module<'a>) }).
            types.push(
            Type::new(self.module, Some(unsafe { &*(self as *const Type<'a>) }))
        );
        let t = (unsafe { &mut *(self.module as *const Module<'a> as *mut Module<'a>) }).types.last_mut().unwrap();
        t.kind = f(unsafe { &*(t as *const Type<'a>) });
        t
    }

    pub fn new_vector(&mut self) -> &mut Vector<'a> {
        match self.push(|t| Kind::Vector(Vector::new(t, 0))).kind {
            Kind::Vector(ref mut v) => v,
            _ => unreachable!(),
        }
    }

    pub fn new_array(&mut self, size: usize) -> &mut Array<'a> {
        match self.push(|t| Kind::Array(Array::new(t, size))).kind {
            Kind::Array(ref mut a) => a,
            _ => unreachable!(),
        }
    }

    pub fn new_array_vector(&mut self, size: usize) -> &mut ArrayVector<'a> {
        match self.push(|t| Kind::ArrayVector(ArrayVector::new(t, size))).kind {
            Kind::ArrayVector(ref mut a) => a,
            _ => unreachable!(),
        }
    }

    fn new_enum(&mut self) -> &mut Enum<'a> {
        match self.push(|t| Kind::Enum(Enum::new(t))).kind {
            Kind::Enum(ref mut e) => e,
            _ => unreachable!(),
        }
    }

    fn new_struct(&mut self, kind: StructKind) -> &mut Struct<'a> {
        match self.push(|t| Kind::Struct(Struct::new(t, kind))).kind {
            Kind::Struct(ref mut s) => s,
            _ => unreachable!(),
        }
    }
    fn new_field(&mut self, parent: &'a Struct<'a>) -> &mut Field<'a> {
        match self.push(|t| Kind::Field(Field::new(t, parent))).kind {
            Kind::Field(ref mut f) => f,
            _ => unreachable!(),
        }
    }

    pub fn new_alias(&mut self, name: String, of: Option<&'a Type<'a>>) -> &mut Alias<'a> {
        match self.push(|t| Kind::Alias(Alias::new(t, name, of))).kind {
            Kind::Alias(ref mut a) => a,
            _ => unreachable!(),
        }
    }
    pub fn new_optional(&mut self, of: Option<&'a Type<'a>>) -> &mut Optional<'a> {
        match self.push(|t| Kind::Optional(Optional::new(t, of))).kind {
            Kind::Optional(ref mut o) => o,
            _ => unreachable!(),
        }
    }
    pub fn new_pointer(&mut self, of: Option<&'a Type<'a>>) -> &mut Pointer<'a> {
        match self.push(|t| Kind::Pointer(Pointer::new(t, of))).kind {
            Kind::Pointer(ref mut p) => p,
            _ => unreachable!(),
        }
    }
    pub fn new_unknown(&mut self, name: Option<String>) -> &mut Unknown<'a> {
        match self.push(|t| Kind::Unknown(Unknown::new(t, name))).kind {
            Kind::Unknown(ref mut n) => n,
            _ => unreachable!(),
        }
    }
    pub fn new_bool(&mut self) -> &mut Type<'a> {
        self.push(|_| Kind::Bool)
    }
    pub fn new_i8(&mut self) -> &mut Type<'a> {
        self.push(|_| Kind::I8)
    }
    pub fn new_i16(&mut self, endian: Endian) -> &mut Number<'a> {
        match self.push(|t| Kind::I16(Number::new(t, endian))).kind {
            Kind::I16(ref mut n) => n,
            _ => unreachable!(),
        }
    }
    pub fn new_i32(&mut self, endian: Endian) -> &mut Number<'a> {
        match self.push(|t| Kind::I32(Number::new(t, endian))).kind {
            Kind::I32(ref mut n) => n,
            _ => unreachable!(),
        }
    }
    pub fn new_i64(&mut self, endian: Endian) -> &mut Number<'a> {
        match self.push(|t| Kind::I64(Number::new(t, endian))).kind {
            Kind::I64(ref mut n) => n,
            _ => unreachable!(),
        }
    }
    pub fn new_i128(&mut self, endian: Endian) -> &mut Number<'a> {
        match self.push(|t| Kind::I128(Number::new(t, endian))).kind {
            Kind::I128(ref mut n) => n,
            _ => unreachable!(),
        }
    }
    pub fn new_u8(&mut self) -> &mut Type<'a> {
        self.push(|_| Kind::U8)
    }
    pub fn new_u16(&mut self, endian: Endian) -> &mut Number<'a> {
        match self.push(|t| Kind::U16(Number::new(t, endian))).kind {
            Kind::U16(ref mut n) => n,
            _ => unreachable!(),
        }
    }
    pub fn new_u32(&mut self, endian: Endian) -> &mut Number<'a> {
        match self.push(|t| Kind::U32(Number::new(t, endian))).kind {
            Kind::U32(ref mut n) => n,
            _ => unreachable!(),
        }
    }
    pub fn new_u64(&mut self, endian: Endian) -> &mut Number<'a> {
        match self.push(|t| Kind::U64(Number::new(t, endian))).kind {
            Kind::U64(ref mut n) => n,
            _ => unreachable!(),
        }
    }
    pub fn new_u128(&mut self, endian: Endian) -> &mut Number<'a> {
        match self.push(|t| Kind::U128(Number::new(t, endian))).kind {
            Kind::U128(ref mut n) => n,
            _ => unreachable!(),
        }
    }
    pub fn new_f32(&mut self, endian: Endian) -> &mut Number<'a> {
        match self.push(|t| Kind::F32(Number::new(t, endian))).kind {
            Kind::F32(ref mut n) => n,
            _ => unreachable!(),
        }
    }
    pub fn new_f64(&mut self, endian: Endian) -> &mut Number<'a> {
        match self.push(|t| Kind::F64(Number::new(t, endian))).kind {
            Kind::F64(ref mut n) => n,
            _ => unreachable!(),
        }
    }

    pub fn new_string(&mut self) -> &mut Str<'a> {
        match self.push(|t| Kind::String(Str::new(t, 0, 0))).kind {
            Kind::String(ref mut s) => s,
            _ => unreachable!(),
        }
    }
    pub fn new_string_inline(&mut self, size: usize) -> &mut Str<'a> {
        match self.push(|t| Kind::StringInline(Str::new(t, size, 0))).kind {
            Kind::StringInline(ref mut s) => s,
            _ => unreachable!(),
        }
    }
    pub fn new_string_inline_plus(&mut self, size: usize) -> &mut Str<'a> {
        match self.push(|t| Kind::StringInlinePlus(Str::new(t, size, 0))).kind {
            Kind::StringInlinePlus(ref mut s) => s,
            _ => unreachable!(),
        }
    }
}

pub trait KindVariant<'a> {
    fn this_ptr(&self) -> *const Type<'a>;

    fn this(&self) -> &'a Type<'a> {
        unsafe { &*self.this_ptr() }
    }

    fn this_mut(&mut self) -> &'a mut Type<'a> {
        unsafe { &mut *(self.this_ptr() as *mut Type<'a>) }
    }

    fn module(&self) -> &Module<'a> {
        self.this().module()
    }

    fn module_mut(&mut self) -> &'a mut Module<'a> {
        (unsafe { &mut *(self.this_ptr() as *mut Type<'a>) }).module_mut()
    }

    fn name(&self) -> &'a str {
        self.this().kind.name()
    }
}

#[derive(Clone)]
pub struct Const<'a> {
    owner: &'a Type<'a>,
    value: Value<'a>,
}

#[derive(Clone)]
pub struct Alias<'a> {
    this: &'a Type<'a>,
    name: String,
    of: Option<&'a Type<'a>>,
}

impl<'a> Alias<'a> {
    pub fn new(this: &'a Type<'a>, name: String, of: Option<&'a Type<'a>>) -> Self {
        Self { this, name, of }
    }

    pub fn set_of(&mut self, of: Option<&'a Type<'a>>) {
        self.of = of;
    }
}

impl<'a> KindVariant<'a> for Alias<'a> {
    fn this_ptr(&self) -> *const Type<'a> {
        self.this
    }
}

#[derive(Clone)]
pub struct Optional<'a> {
    this: &'a Type<'a>,
    of: Option<&'a Type<'a>>,
}

impl<'a> Optional<'a> {
    pub fn new(this: &'a Type<'a>, of: Option<&'a Type<'a>>) -> Self {
        Self { this, of }
    }

    pub fn set_of(&mut self, of: Option<&'a Type<'a>>) {
        self.of = of;
    }
}

impl<'a> KindVariant<'a> for Optional<'a> {
    fn this_ptr(&self) -> *const Type<'a> {
        self.this
    }
}

#[derive(Clone)]
pub struct Pointer<'a> {
    this: &'a Type<'a>,
    of: Option<&'a Type<'a>>,
}

impl<'a> Pointer<'a> {
    pub fn new(this: &'a Type<'a>, of: Option<&'a Type<'a>>) -> Self {
        Self { this, of }
    }

    pub fn set_of(&mut self, of: Option<&'a Type<'a>>) {
        self.of = of;
    }
}

impl<'a> KindVariant<'a> for Pointer<'a> {
    fn this_ptr(&self) -> *const Type<'a> {
        self.this
    }
}

#[derive(Copy, Clone, PartialEq)]
pub enum Endian {
    Little = 0,
    Big = 1,
    Native = 2,
}

#[derive(Clone)]
pub struct Unknown<'a> {
    this: &'a Type<'a>,
    name: Option<String>,
}

impl<'a> Unknown<'a> {
    pub fn new(this: &'a Type<'a>, name: Option<String>) -> Self {
        Self { this, name }
    }
}

impl<'a> KindVariant<'a> for Unknown<'a> {
    fn this_ptr(&self) -> *const Type<'a> {
        self.this
    }
}

#[derive(Clone)]
pub struct Number<'a> {
    this: &'a Type<'a>,
    endian: Endian,
}

impl<'a> Number<'a> {
    fn new(this: &'a Type<'a>, endian: Endian) -> Self {
        Self { this, endian }
    }
}

impl<'a> KindVariant<'a> for Number<'a> {
    fn this_ptr(&self) -> *const Type<'a> {
        self.this
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

impl<'a> KindVariant<'a> for Str<'a> {
    fn this_ptr(&self) -> *const Type<'a> {
        self.owner
    }
}

#[derive(Clone, PartialEq)]
pub enum StructKind {
    Struct,
    Variant,
    Union,
}

#[derive(Clone)]
pub struct Struct<'a> {
    this: &'a Type<'a>,
    name: Option<String>,
    align: u16,
    pack: u16,
    size: u32,
    kind: StructKind,
    fields: Vec<&'a Field<'a>>,
    fields_sorted: Vec<&'a Field<'a>>,
    inner: Option<Vec<&'a Type<'a>>>,
}

impl<'a> Struct<'a> {
    fn new(this: &'a Type<'a>, kind: StructKind) -> Self {
        Self {
            this,
            name: None,
            align: 0,
            pack: 0,
            size: 0,
            kind: StructKind::Struct,
            fields: Vec::new(),
            fields_sorted: Vec::new(),
            inner: None,
        }
    }

    pub fn set_name(&mut self, name: String) {
        self.name = Some(name);
    }
    pub fn set_align(&mut self, align: u16) {
        self.align = align;
    }
    pub fn set_pack(&mut self, pack: u16) {
        self.pack = pack;
    }
    pub fn set_size(&mut self, size: u32) {
        self.size = size;
    }
    pub fn set_kind(&mut self, kind: StructKind) {
        self.kind = kind;
    }

    pub fn new_field(&mut self) -> &mut Field<'a> {
        let f = self.this_mut().new_field(unsafe { &*(self as *const Self) });
        self.fields.push(unsafe { &*(f as *const Field<'a>) });
        f
    }

    pub fn new_enum(&mut self) -> &mut Enum<'a> {
        let e = self.this_mut().new_enum();
        if self.inner.is_none() {
            self.inner = Some(vec![e.this()])
        } else {
            let mut vector = self.inner.take().unwrap();
            vector.push(e.this());
            self.inner = Some(vector);
        }
        e
    }

    fn new_struct_kind(&mut self, kind: StructKind) -> &mut Struct<'a> {
        let s = self.this_mut().new_struct(kind);
        if self.inner.is_none() {
            self.inner = Some(vec![s.this()])
        } else {
            let mut vector = self.inner.take().unwrap();
            vector.push(s.this());
            self.inner = Some(vector);
        }
        s
    }

    pub fn new_struct(&mut self) -> &mut Struct<'a> {
        self.new_struct_kind(StructKind::Struct)
    }

    pub fn new_union(&mut self) -> &mut Struct<'a> {
        self.new_struct_kind(StructKind::Union)
    }

    pub fn new_variant(&mut self) -> &mut Struct<'a> {
        self.new_struct_kind(StructKind::Variant)
    }
}

impl<'a> KindVariant<'a> for Struct<'a> {
    fn this_ptr(&self) -> *const Type<'a> {
        self.this
    }
}

#[derive(Clone)]
pub struct Field<'a> {
    this: &'a Type<'a>,
    parent: &'a Struct<'a>,
    name: Option<String>,
    number: usize,
}

impl<'a> Field<'a> {
    pub fn new(this: &'a Type<'a>, parent: &'a Struct<'a>) -> Self {
        Self { this, parent, name: None, number: 0 }
    }
}

#[derive(Clone)]
pub struct Enum<'a> {
    this: &'a Type<'a>,
}

impl<'a> Enum<'a> {
    fn new(this: &'a Type<'a>) -> Self {
        Self { this }
    }
}

impl<'a> KindVariant<'a> for Enum<'a> {
    fn this_ptr(&self) -> *const Type<'a> {
        self.this
    }
}

#[derive(Clone)]
pub struct EnumOption<'a> {
    this: &'a Type<'a>,
    value: &'a Value<'a>,
}

#[derive(Clone)]
pub struct Array<'a> {
    this: &'a Type<'a>,
    element: Option<&'a Type<'a>>,
    size: usize
}

impl<'a> Array<'a> {
    fn new(this: &'a Type<'a>, size: usize) -> Self {
        Self {
            this,
            element: None,
            size
        }
    }

    pub fn set_element(&mut self, element: Option<&'a Type<'a>>) {
        self.element = element;
    }
}

impl<'a> KindVariant<'a> for Array<'a> {
    fn this_ptr(&self) -> *const Type<'a> {
        self.this
    }
}

#[derive(Clone)]
pub struct ArrayVector<'a> {
    this: &'a Type<'a>,
    element: Option<&'a Type<'a>>,
    array_size: usize
}

impl<'a> ArrayVector<'a> {
    fn new(this: &'a Type<'a>, array_size: usize) -> Self {
        Self {
            this,
            element: None,
            array_size
        }
    }

    pub fn set_element(&mut self, element: Option<&'a Type<'a>>) {
        self.element = element;
    }
}

impl<'a> KindVariant<'a> for ArrayVector<'a> {
    fn this_ptr(&self) -> *const Type<'a> {
        self.this
    }
}

#[derive(Clone)]
pub struct Vector<'a> {
    this: &'a Type<'a>,
    element: Option<&'a Type<'a>>,
    size: usize
}

impl<'a> Vector<'a> {
    fn new(this: &'a Type<'a>, size: usize) -> Self {
        Self {
            this,
            element: None,
            size
        }
    }

    pub fn set_element(&mut self, element: Option<&'a Type<'a>>) {
        self.element = element;
    }
}

impl<'a> KindVariant<'a> for Vector<'a> {
    fn this_ptr(&self) -> *const Type<'a> {
        self.this
    }
}

#[derive(Clone)]
pub struct Map<'a> {
    owner: &'a Type<'a>,
    key: Option<&'a Type<'a>>,
    value: Option<&'a Type<'a>>
}

impl<'a> Map<'a> {
    fn new(owner: &'a Type<'a>) -> Self {
        Self {
            owner,
            key: None,
            value: None
        }
    }

    pub fn set_key(&mut self, key: Option<&'a Type<'a>>) {
        self.key = key;
    }

    pub fn set_value(&mut self, value: Option<&'a Type<'a>>) {
        self.value = value;
    }
}

impl<'a> KindVariant<'a> for Map<'a> {
    fn this_ptr(&self) -> *const Type<'a> {
        self.owner
    }
}


#[derive(Clone)]
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
    Enum(&'a EnumOption<'a>),
    Const(&'a Const<'a>),
    Object(Object<'a>),
}

#[derive(Clone)]
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

#[derive(Clone)]
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

        println!("done");
    }
}