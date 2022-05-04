use std::{mem, ptr};
use std::borrow::BorrowMut;
use std::cell::RefCell;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::ptr::{null, null_mut};
use std::rc::{Rc, Weak};

use anyhow::Error;

#[derive(Copy, Clone, PartialEq)]
pub enum TypeCode {
    Unknown = 0,
    Bool = 1,
    I8 = 2,
    U8 = 3,
    I16 = 4,
    U16 = 5,
    I32 = 6,
    U32 = 7,
    I64 = 8,
    U64 = 9,
    I128 = 10,
    U128 = 11,
    F32 = 12,
    F64 = 13,
    F128 = 14,
    String = 20,
    // heap allocated string
    StringInline = 21,
    // string embedded inline
    StringInlinePlus = 22,
    // string embedded inline with a heap allocated spill-over
    Enum = 25,
    Struct = 30,
    Union = 31,
    Variant = 32,
    Optional = 33,
    Pointer = 34,
    Array = 40,
    Vector = 41,
    Map = 50,
    // robin-hood map
    MapOrdered = 51,
    // ART radix tree
    MapTree = 52,
    // B+Tree
    Set = 60,
    SetOrdered = 61,
    SetTree = 62,
    Const = 90,
    Pad = 100,
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Position {
    pub line: usize,
    pub col: usize,
    pub index: usize,
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub struct Comment<'a> {
    pub begin: Position,
    pub end: Position,
    pub inline: bool,
    pub value: &'a str,
}

pub struct CommentLine {
    pub begin: Position,
    pub end: Position,
}

pub struct RefMut<'a, T>(*mut T, PhantomData<(&'a ())>);

impl<'a, T> RefMut<'a, T> {
    pub fn new(r: *mut T) -> Self {
        Self(r, PhantomData)
    }
}

impl<'a, T> Deref for RefMut<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.0 }
    }
}

impl<'a, T> DerefMut for RefMut<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.0 }
    }
}

impl<'a, T> Clone for RefMut<'a, T> {
    fn clone(&self) -> Self {
        Self(self.0, PhantomData)
    }
}

pub struct Module<'a> {
    pub contents: String,
    pub full: String,
    pub name: String,
    // Type storage
    types: Vec<Type<'a>>,
    // Named type index
    types_map: HashMap<&'a str, &'a Type<'a>>,
    // Value storage.
    values: Vec<Value<'a>>,
    _marker: PhantomData<(&'a ())>,
}

impl<'a> Module<'a> {
    pub fn new(contents: String, full: String, name: String) -> Self {
        Self {
            contents,
            full,
            name,
            types: Vec::new(),
            types_map: HashMap::new(),
            values: Vec::new(),
            _marker: PhantomData,
        }
    }

    pub fn types_iter(&self) -> impl Iterator<Item=&Type<'a>> {
        self.types.iter()
    }

    fn push_type<T: TypeMeta<'a>, R>(&mut self,
                                     parent: *mut Type<'a>,
                                     code: TypeCode,
                                     f: impl FnOnce(
                                         &mut Module<'a>,
                                         &mut Type<'a>,
                                         &mut T,
                                     ) -> anyhow::Result<R>) -> anyhow::Result<R> {
        self.types.push(Type::new(self, parent, None, code));
        unsafe { T::create(self.types.last_mut().unwrap(), f) }
    }

    pub fn new_struct<R>(&mut self,
                         f: impl FnOnce(
                             &mut Module<'a>,
                             &mut Type<'a>,
                             &mut Struct<'a>,
                         ) -> anyhow::Result<R>) -> anyhow::Result<R> {
        self.new_struct_with_parent(null_mut(), f)
    }

    pub fn new_struct_with_parent<R>(&mut self,
                                     parent: *mut Type<'a>,
                                     f: impl FnOnce(
                                         &mut Module<'a>,
                                         &mut Type<'a>,
                                         &mut Struct<'a>,
                                     ) -> anyhow::Result<R>) -> anyhow::Result<R> {
        self.types.push(Type::new(self, parent, None, TypeCode::Struct));
        unsafe { Struct::<'a>::create(self.types.last_mut().unwrap(), f) }
    }
}

#[derive(Clone)]
pub struct Import<'a> {
    module: &'a Module<'a>,
    alias: String,
}

pub struct Type<'a> {
    pub name: Option<String>,
    pub code: TypeCode,
    pub info: TypeInfo<'a>,
    module: *const Module<'a>,
    parent: *const Type<'a>,
    value: Option<*const Value<'a>>,
    inner: Option<Vec<*const Type<'a>>>,
}

const EMPTY: &'static str = "";
const NAME_I8: &'static str = "i8";
const NAME_U8: &'static str = "u8";

impl<'a> Type<'a> {
    fn new(
        module: *const Module<'a>,
        parent: *const Type<'a>,
        name: Option<String>,
        code: TypeCode,
    ) -> Self {
        Self {
            name,
            code,
            module,
            parent,
            info: TypeInfo::None,
            value: None,
            inner: None,
        }
    }

    pub fn new_struct<R>(&mut self,
                         f: impl FnOnce(
                             &mut Module<'a>,
                             &mut Type<'a>,
                             &mut Struct<'a>,
                         ) -> anyhow::Result<R>) -> anyhow::Result<R> {
        let this = unsafe { self as *mut Self };
        self.module_mut().push_type(this, TypeCode::Struct, f)
    }

    pub fn full_name(&self) -> Option<String> {
        match self.info {
            TypeInfo::None => {}
            TypeInfo::Optional(_) => {}
            TypeInfo::Pointer(_) => {}
            TypeInfo::Number { .. } => {}
            TypeInfo::String { .. } => {}
            TypeInfo::Struct(_) => {}
            TypeInfo::Field(_) => {}
            TypeInfo::Union(_) => {}
            TypeInfo::Variant(_) => {}
            TypeInfo::Enum(_) => {}
            TypeInfo::EnumOption(_) => {}
        }
        None
    }

    pub fn module(&self) -> &Module<'a> {
        unsafe { &*self.module }
    }

    pub fn module_mut(&mut self) -> &mut Module<'a> {
        unsafe { &mut *(self.module as *mut Module) }
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

    pub fn set_info(&mut self, info: TypeInfo<'a>) -> &mut Self {
        self.info = info;
        self
    }
}

pub trait TypeMeta<'a> {
    fn new(owner: &'a mut Type<'a>) -> TypeInfo<'a>;

    unsafe fn get_mut(t: *mut Type<'a>) -> *mut Self;

    unsafe fn create<T>(
        owner: *mut Type<'a>,
        f: impl FnOnce(&mut Module<'a>, &mut Type<'a>, &mut Self) -> anyhow::Result<T>) -> anyhow::Result<T> {
        let o = &mut *owner;
        o.info = Self::new(&mut *owner);
        let info = &mut *(Self::get_mut(&mut *(owner)));
        f((&mut *owner).module_mut(), &mut *owner, info)
    }

    fn owner_ptr(&self) -> *const Type;

    fn code(&self) -> TypeCode {
        self.owner().code
    }

    fn module(&self) -> &Module {
        self.owner().module()
    }

    fn module_mut(&mut self) -> &mut Module {
        self.owner_mut().module_mut()
    }

    fn owner(&self) -> &Type {
        unsafe { &*self.owner_ptr() }
    }

    fn owner_mut(&mut self) -> &mut Type {
        unsafe { &mut *(self.owner_ptr() as *mut Type) }
    }
}

#[derive(Clone)]
pub enum TypeInfo<'a> {
    None,
    Optional(Optional<'a>),
    Pointer(Pointer<'a>),
    Number(Number<'a>),
    String(Str<'a>),
    Struct(Struct<'a>),
    Field(Field<'a>),
    Union(Union<'a>),
    Variant(Variant<'a>),
    Enum(Enum<'a>),
    EnumOption(EnumOption<'a>),
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

#[derive(Copy, Clone)]
pub enum Endian {
    Little = 0,
    Big = 1,
    Native = 2,
}

#[derive(Clone)]
pub struct Number<'a> {
    owner: *const Type<'a>,
    endian: Endian,
}

#[derive(Clone)]
pub struct Str<'a> {
    owner: *const Type<'a>,
    inline_size: usize,
    max_size: usize,
    inline: bool,
}

#[derive(Clone)]
pub struct Struct<'a> {
    owner: *const Type<'a>,
    align: u16,
    pack: u16,
    size: u32,
    fields: Vec<*const Type<'a>>,
}

impl<'a> TypeMeta<'a> for Struct<'a> {
    fn new(owner: &'a mut Type<'a>) -> TypeInfo<'a> {
        TypeInfo::Struct(Self {
            owner,
            align: 0,
            pack: 0,
            size: 0,
            fields: Vec::new(),
        })
    }

    unsafe fn get_mut(t: *mut Type<'a>) -> *mut Self {
        match (&mut *t).info {
            TypeInfo::Struct(ref mut s) => s,
            _ => panic!("expected struct")
        }
    }

    fn owner_ptr(&self) -> *const Type {
        self.owner
    }
}

#[derive(Clone)]
pub struct Field<'a> {
    owner: *const Type<'a>,
}

#[derive(Clone)]
pub struct Union<'a> {
    owner: *const Type<'a>,
}

#[derive(Clone)]
pub struct Variant<'a> {
    owner: *const Type<'a>,
}

#[derive(Clone)]
pub struct Enum<'a> {
    owner: *const Type<'a>,
}

#[derive(Clone)]
pub struct EnumOption<'a> {
    owner: *const Type<'a>,
    value: *const Value<'a>,
}

#[derive(Clone)]
pub struct Vector<'a> {
    owner: *const Type<'a>,
    element: *const Type<'a>,
}

#[derive(Clone)]
pub struct Map<'a> {
    owner: *const Type<'a>,
    key: *const Type<'a>,
    value: *const Type<'a>,
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
    EnumOption(EnumOption<'a>),
    Object,
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
        let mut m = Module::new(String::from(""), String::from("wap::model"), String::from("model"));

        m.new_struct(|mut m, t, s| {
            m.full = String::from("");
            t.name = Some(String::from("Order"));

            t.new_struct(|_, t, s| {
                Ok(())
            });

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