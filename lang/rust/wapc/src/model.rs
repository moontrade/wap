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

#[derive(Clone)]
pub enum Kind<'a> {
    Module(*const Module<'a>),
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
    fn set_owner(&mut self, owner: *const Type<'a>) {
        match self {
            Kind::Module(..) => {}
            Kind::Unknown => {}
            Kind::Bool => {}
            Kind::I8 => {}
            Kind::U8 => {}
            Kind::I16(v) => v.owner = owner,
            Kind::U16(v) => v.owner = owner,
            Kind::I32(v) => v.owner = owner,
            Kind::U32(v) => v.owner = owner,
            Kind::I64(v) => v.owner = owner,
            Kind::U64(v) => v.owner = owner,
            Kind::I128(v) => v.owner = owner,
            Kind::U128(v) => v.owner = owner,
            Kind::F32(v) => v.owner = owner,
            Kind::F64(v) => v.owner = owner,
            Kind::F128(v) => v.owner = owner,
            Kind::String(v) => v.owner = owner,
            Kind::StringInline(v) => v.owner = owner,
            Kind::StringInlinePlus(v) => v.owner = owner,
            Kind::Optional(v) => v.owner = owner,
            Kind::Pointer(v) => v.owner = owner,
            Kind::Enum(v) => v.owner = owner,
            Kind::EnumOption(v) => v.owner = owner,
            Kind::Struct(v) => v.owner = unsafe { &*owner },
            Kind::StructField(v) => v.owner = owner,
            Kind::Union(v) => v.owner = owner,
            Kind::UnionField(v) => v.owner = owner,
            Kind::Variant(v) => v.owner = owner,
            Kind::VariantOption(v) => v.owner = owner,
            Kind::Array(v) => v.owner = owner,
            Kind::Vector(v) => v.owner = owner,
            Kind::FlatMap(v) => v.owner = owner,
            Kind::NodeMap(v) => v.owner = owner,
            Kind::FlatSet(v) => v.owner = owner,
            Kind::NodeSet(v) => v.owner = owner,
            Kind::OrderedMap(v) => v.owner = owner,
            Kind::OrderedSet(v) => v.owner = owner,
            Kind::TreeMap(v) => v.owner = owner,
            Kind::TreeSet(v) => v.owner = owner,
            Kind::Const(v) => v.owner = owner,
            Kind::Alias(v) => {}
            Kind::Pad(v) => {}
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
    types_map: BTreeMap<&'a str, &'a Type<'a>>,
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
            types_map: BTreeMap::new(),
            values: Vec::new(),
            _marker: PhantomData,
        });
        m.root = Type::new(m.as_ref(), null(), None);
        m.root.kind = Kind::Module(m.as_ref());
        m
    }

    pub fn types_iter(&self) -> impl Iterator<Item=&Type<'a>> {
        self.types.iter()
    }

    fn push(&mut self, mut info: Kind<'a>) -> &mut Type<'a> {
        self.push_with_parent(null(), info)
    }

    fn push_with_parent(&mut self, parent: *const Type<'a>, mut info: Kind<'a>) -> &mut Type<'a> {
        self.types.push(Type::new(self, null(), None));
        let t = self.types.last_mut().unwrap();
        t.parent = parent;
        t.kind = info;
        t.kind.set_owner(t);
        t
    }

    fn push_with_parent0(&mut self, parent: *const Type<'a>,
                         f: impl FnOnce(&'a mut Type<'a>) -> Kind<'a>,
    ) -> &'a mut Type<'a> {
        self.types.push(Type::new(self, null(), None));
        let mut t = self.types.last_mut().unwrap();
        Mut::new(self).types.last_mut().unwrap().kind = f(t);
        // t.parent = parent;
        // t.kind.set_owner(t);
        t
    }

    pub fn new_enum<R>(&mut self,
                       f: impl FnOnce(
                           &mut Enum<'a>,
                       ) -> anyhow::Result<R>) -> anyhow::Result<R> {
        self.new_enum_with_parent(null_mut(), f)
    }

    pub fn new_enum_with_parent<R>(&mut self,
                                   parent: *mut Type<'a>,
                                   f: impl FnOnce(
                                       &mut Enum<'a>,
                                   ) -> anyhow::Result<R>) -> anyhow::Result<R> {
        match self.push_with_parent(parent, Kind::Enum(Enum::new())).kind {
            Kind::Enum(ref mut s) => f(s),
            _ => panic!("unreachable"),
        }
    }

    pub fn new_struct<R>(&mut self,
                         f: impl FnOnce(
                             &mut Struct<'a>,
                         ) -> anyhow::Result<R>) -> anyhow::Result<R> {
        self.new_struct_with_parent(null_mut(), f)
    }

    pub fn new_struct_with_parent<R>(&mut self,
                                     parent: *mut Type<'a>,
                                     f: impl FnOnce(
                                         &mut Struct<'a>,
                                     ) -> anyhow::Result<R>) -> anyhow::Result<R> {
        match self.push_with_parent0(parent, |t| Kind::Struct(Struct::new(t))).kind {
            Kind::Struct(ref mut s) => f(s),
            _ => panic!("unreachable"),
        }
    }

    pub fn new_union<R>(&mut self,
                        f: impl FnOnce(
                            &mut Union<'a>,
                        ) -> anyhow::Result<R>) -> anyhow::Result<R> {
        self.new_union_with_parent(null_mut(), f)
    }

    pub fn new_union_with_parent<R>(&mut self,
                                    parent: *mut Type<'a>,
                                    f: impl FnOnce(
                                        &mut Union<'a>,
                                    ) -> anyhow::Result<R>) -> anyhow::Result<R> {
        match self.push_with_parent(parent, Kind::Union(Union::new())).kind {
            Kind::Union(ref mut s) => f(s),
            _ => panic!("unreachable"),
        }
    }

    pub fn new_variant<R>(&mut self,
                          f: impl FnOnce(
                              &mut Variant<'a>,
                          ) -> anyhow::Result<R>) -> anyhow::Result<R> {
        self.new_variant_with_parent(null_mut(), f)
    }

    fn new_variant_with_parent<R>(&mut self,
                                  parent: *mut Type<'a>,
                                  f: impl FnOnce(
                                      &mut Variant<'a>,
                                  ) -> anyhow::Result<R>) -> anyhow::Result<R> {
        match self.push_with_parent(parent, Kind::Variant(Variant::new())).kind {
            Kind::Variant(ref mut s) => f(s),
            _ => panic!("unreachable"),
        }
    }

    fn new_map_with_parent<R>(&mut self,
                              parent: *mut Type<'a>,
                              f: impl FnOnce(
                                  &mut Map<'a>,
                              ) -> anyhow::Result<R>) -> anyhow::Result<R> {
        match self.push_with_parent(parent, Kind::FlatMap(Map::new())).kind {
            Kind::FlatMap(ref mut s) => f(s),
            _ => panic!("unreachable"),
        }
    }

    fn new_node_map_with_parent<R>(&mut self,
                                   parent: *mut Type<'a>,
                                   f: impl FnOnce(
                                       &mut Map<'a>,
                                   ) -> anyhow::Result<R>) -> anyhow::Result<R> {
        match self.push_with_parent(parent, Kind::NodeMap(Map::new())).kind {
            Kind::NodeMap(ref mut s) => f(s),
            _ => panic!("unreachable"),
        }
    }

    fn new_ordered_map_with_parent<R>(&mut self,
                                      parent: *mut Type<'a>,
                                      f: impl FnOnce(
                                          &mut Map<'a>,
                                      ) -> anyhow::Result<R>) -> anyhow::Result<R> {
        match self.push_with_parent(parent, Kind::OrderedMap(Map::new())).kind {
            Kind::OrderedMap(ref mut s) => f(s),
            _ => panic!("unreachable"),
        }
    }

    fn new_tree_map_with_parent<R>(&mut self,
                                   parent: *mut Type<'a>,
                                   f: impl FnOnce(
                                       &mut Map<'a>,
                                   ) -> anyhow::Result<R>) -> anyhow::Result<R> {
        match self.push_with_parent(parent, Kind::TreeMap(Map::new())).kind {
            Kind::TreeMap(ref mut s) => f(s),
            _ => panic!("unreachable"),
        }
    }

    fn new_set_with_parent<R>(&mut self,
                              parent: *mut Type<'a>,
                              f: impl FnOnce(
                                  &mut Map<'a>,
                              ) -> anyhow::Result<R>) -> anyhow::Result<R> {
        match self.push_with_parent(parent, Kind::FlatSet(Map::new())).kind {
            Kind::FlatSet(ref mut s) => f(s),
            _ => panic!("unreachable"),
        }
    }

    fn new_node_set_with_parent<R>(&mut self,
                                   parent: *mut Type<'a>,
                                   f: impl FnOnce(
                                       &mut Map<'a>,
                                   ) -> anyhow::Result<R>) -> anyhow::Result<R> {
        match self.push_with_parent(parent, Kind::NodeSet(Map::new())).kind {
            Kind::NodeSet(ref mut s) => f(s),
            _ => panic!("unreachable"),
        }
    }

    fn new_ordered_set_with_parent<R>(&mut self,
                                      parent: *mut Type<'a>,
                                      f: impl FnOnce(
                                          &mut Map<'a>,
                                      ) -> anyhow::Result<R>) -> anyhow::Result<R> {
        match self.push_with_parent(parent, Kind::OrderedSet(Map::new())).kind {
            Kind::OrderedSet(ref mut s) => f(s),
            _ => panic!("unreachable"),
        }
    }

    fn new_tree_set_with_parent<R>(&mut self,
                                   parent: *mut Type<'a>,
                                   f: impl FnOnce(
                                       &mut Map<'a>,
                                   ) -> anyhow::Result<R>) -> anyhow::Result<R> {
        match self.push_with_parent(parent, Kind::TreeSet(Map::new())).kind {
            Kind::TreeSet(ref mut s) => f(s),
            _ => panic!("unreachable"),
        }
    }

    // pub fn new_unknown(&mut self, parent: ) -> &mut Type<'a> {
    //     self.push(TypeCode::Unknown)
    // }
}

#[derive(Clone)]
pub struct Import<'a> {
    module: &'a Module<'a>,
    alias: String,
}

pub struct Type<'a> {
    pub name: Option<String>,
    pub kind: Kind<'a>,
    module: *const Module<'a>,
    parent: *const Type<'a>,
    value: Option<*const Value<'a>>,
}

const EMPTY: &'static str = "";

impl<'a> Type<'a> {
    fn new(
        module: *const Module<'a>,
        parent: *const Type<'a>,
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


    pub fn module(&self) -> &Module<'a> {
        unsafe { &*self.module }
    }

    pub fn module_mut(&'a mut self) -> &'a mut Module<'a> {
        unsafe { &mut *(self.module as *mut Module<'a>) }
    }

    fn parent(&self) -> Option<&Type<'a>> {
        let owner = self.parent;
        if owner == null() {
            None
        } else {
            Some(unsafe { &*owner })
        }
    }

    fn parent_mut(&mut self) -> Option<&mut Type<'a>> {
        let owner = unsafe { self.parent as *mut Type };
        if owner == null_mut() {
            None
        } else {
            Some(unsafe { &mut *owner })
        }
    }

    pub fn name(&self) -> &str {
        match &self.name {
            Some(ref s) => s.as_str(),
            None => &EMPTY,
        }
    }

    pub fn new_enum<R>(&'a mut self,
                       f: impl FnOnce(
                           &mut Enum<'a>,
                       ) -> anyhow::Result<R>) -> anyhow::Result<R> {
        let parent = unsafe { self as *mut Self };
        self.module_mut().new_enum_with_parent(parent, f)
    }

    pub fn new_struct<R>(&'a mut self,
                         f: impl FnMut(
                             &mut Struct<'a>,
                         ) -> anyhow::Result<R>) -> anyhow::Result<R> {
        let parent = unsafe { self as *mut Self };
        self.module_mut().new_struct_with_parent(parent, f)
    }

    pub fn new_union<R>(&'a mut self,
                        f: impl FnOnce(
                            &mut Union<'a>,
                        ) -> anyhow::Result<R>) -> anyhow::Result<R> {
        let parent = unsafe { self as *mut Self };
        self.module_mut().new_union_with_parent(parent, f)
    }

    pub fn new_variant<R>(&'a mut self,
                          f: impl FnOnce(
                              &mut Variant<'a>,
                          ) -> anyhow::Result<R>) -> anyhow::Result<R> {
        let parent = unsafe { self as *mut Self };
        self.module_mut().new_variant_with_parent(parent, f)
    }

    pub fn new_string<R>(&'a mut self,
                         inline_size: usize,
                         max_size: usize, ) -> &mut Str<'a> {
        let parent = unsafe { self as *mut Self };
        let t = self.module_mut().push_with_parent(parent, Kind::String(Str::new(inline_size, max_size)));
        match t.kind {
            Kind::String(ref mut s) => s,
            _ => panic!("unreachable")
        }
    }

    pub fn new_unknown(&'a mut self) -> &mut Type<'a> {
        self.module_mut().push(Kind::Unknown)
    }
    pub fn new_bool(&'a mut self) -> &mut Type<'a> {
        self.module_mut().push(Kind::Bool)
    }
    pub fn new_u8(&'a mut self) -> &mut Type<'a> {
        self.module_mut().push(Kind::U8)
    }
    pub fn new_u16(&'a mut self, endian: Endian) -> &mut Number<'a> {
        self.new_number(Kind::U16(Number::new(endian)))
    }
    pub fn new_u32(&'a mut self, endian: Endian) -> &mut Number<'a> {
        self.new_number(Kind::U32(Number::new(endian)))
    }
    pub fn new_u64(&'a mut self, endian: Endian) -> &mut Number<'a> {
        self.new_number(Kind::U64(Number::new(endian)))
    }
    pub fn new_u128(&'a mut self, endian: Endian) -> &mut Number<'a> {
        self.new_number(Kind::U128(Number::new(endian)))
    }
    pub fn new_i8(&'a mut self) -> &mut Type<'a> {
        self.module_mut().push(Kind::I8)
    }
    pub fn new_i16(&'a mut self, endian: Endian) -> &mut Number<'a> {
        let parent = unsafe { self as *mut Self };
        self.new_number(Kind::I16(Number::new(endian)))
    }
    pub fn new_i32(&'a mut self, endian: Endian) -> &mut Number<'a> {
        let parent = unsafe { self as *mut Self };
        self.new_number(Kind::I32(Number::new(endian)))
    }
    pub fn new_i64(&'a mut self, endian: Endian) -> &mut Number<'a> {
        let parent = unsafe { self as *mut Self };
        self.new_number(Kind::I64(Number::new(endian)))
    }
    pub fn new_i128(&'a mut self, endian: Endian) -> &mut Number<'a> {
        self.new_number(Kind::I128(Number::new(endian)))
    }
    pub fn new_f32(&'a mut self, endian: Endian) -> &mut Number<'a> {
        self.new_number(Kind::F32(Number::new(endian)))
    }
    pub fn new_f64(&'a mut self, endian: Endian) -> &mut Number<'a> {
        self.new_number(Kind::F64(Number::new(endian)))
    }

    fn new_number(&'a mut self, info: Kind<'a>) -> &mut Number<'a> {
        let parent = unsafe { self as *mut Self };
        let t = self.module_mut().push_with_parent(parent, info);
        match t.kind {
            Kind::I16(ref mut v) => v,
            Kind::I32(ref mut v) => v,
            Kind::I64(ref mut v) => v,
            Kind::I128(ref mut v) => v,
            Kind::U16(ref mut v) => v,
            Kind::U32(ref mut v) => v,
            Kind::U64(ref mut v) => v,
            Kind::U128(ref mut v) => v,
            Kind::F32(ref mut v) => v,
            Kind::F64(ref mut v) => v,
            _ => panic!("unreachable")
        }
    }
}

pub trait KindData<'a> {
    fn owner_ptr(&self) -> *const Type<'a>;

    fn module(&self) -> &Module<'a> {
        self.owner().module()
    }

    fn module_mut(&mut self) -> &mut Module<'a> {
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
    owner: *const Type<'a>,
    inner: *const Type<'a>,
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

#[derive(Clone, PartialEq)]
pub struct Str<'a> {
    owner: *const Type<'a>,
    inline_size: usize,
    max_size: usize,
}

impl<'a> Str<'a> {
    fn new(inline_size: usize, max_size: usize) -> Self {
        Self {
            owner: null(),
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
    fields: Vec<*const StructField<'a>>,
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
    owner: *const Type<'a>,
}

#[derive(Clone)]
pub struct Union<'a> {
    owner: *const Type<'a>,
    fields: Vec<*const UnionField<'a>>,
    inner: Option<Vec<&'a Type<'a>>>,
}

impl<'a> Union<'a> {
    fn new() -> Self {
        Self {
            owner: null(),
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
    owner: *const Type<'a>,
    inner: Option<Vec<&'a Type<'a>>>,
}

impl<'a> Variant<'a> {
    fn new() -> Self {
        Self {
            owner: null(),
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
    fn new() -> Self {
        Self {
            owner: null(),
        }
    }
}

impl<'a> KindData<'a> for Enum<'a> {
    fn owner_ptr(&self) -> *const Type<'a> {
        self.owner
    }
}

#[derive(Clone, PartialEq)]
pub struct EnumOption<'a> {
    owner: *const Type<'a>,
    value: *const Value<'a>,
}

#[derive(Clone, PartialEq)]
pub struct Vector<'a> {
    owner: *const Type<'a>,
    element: *const Type<'a>,
}

impl<'a> Vector<'a> {
    fn new() -> Self {
        Self {
            owner: null(),
            element: null(),
        }
    }
}

impl<'a> KindData<'a> for Vector<'a> {
    fn owner_ptr(&self) -> *const Type<'a> {
        self.owner
    }
}

#[derive(Clone, PartialEq)]
pub struct Map<'a> {
    owner: *const Type<'a>,
    key: *const Type<'a>,
    value: *const Type<'a>,
}

impl<'a> Map<'a> {
    fn new() -> Self {
        Self {
            owner: null(),
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

        m.new_struct(|s| {
            // s.module_mut().full = String::from("");
            s.owner_mut().name = Some(String::from("Order"));
            s.module_mut();

            // s.owner_mut().new_struct(|s| {
            //     Ok(())
            // });

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
}