use crate::hash::Hasher;

pub trait Map {
    type Key: Hasher;
    type Value;
}