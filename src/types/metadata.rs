
use std::collections::HashMap;
use std::marker::PhantomData;
use std::io;
use std::io::{Read, Write};
use std::fmt;
use protocol;
use protocol::{Serializable};
use format;
use item;

pub struct MetadataKey<T: MetaValue> {
    index: i32,
    ty: PhantomData<T>,
}

impl <T: MetaValue> MetadataKey<T> {
    #[allow(dead_code)]
    // TODO: Make const later when possible
    /*const*/ fn new(index: i32) -> MetadataKey<T> {
        MetadataKey {
            index: index,
            ty: PhantomData,
        }
    }
}

pub struct Metadata {
    map: HashMap<i32, Value>,
}

impl Metadata {
    pub fn new() -> Metadata {
        Metadata { map: HashMap::new() }
    }

    pub fn get<T: MetaValue>(&self, key: &MetadataKey<T>) -> Option<&T> {
        self.map.get(&key.index).map(T::unwrap)
    }

    pub fn put<T: MetaValue>(&mut self, key: &MetadataKey<T>, val: T) {
        self.map.insert(key.index, val.wrap());
    }

    fn put_raw<T: MetaValue>(&mut self, index: i32, val: T) {
        self.map.insert(index, val.wrap());
    }
}

impl Serializable for Metadata {

    fn read_from(buf: &mut io::Read) -> Result<Self, io::Error> {
        let mut m = Metadata::new();
        loop {
            let index = try!(u8::read_from(buf)) as i32;
            if index == 0xFF {
                break;
            }
            let ty = try!(u8::read_from(buf));
            match ty {
                0 => m.put_raw(index, try!(i8::read_from(buf))),
                1 => m.put_raw(index, try!(protocol::VarInt::read_from(buf)).0),
                2 => m.put_raw(index, try!(f32::read_from(buf))),
                3 => m.put_raw(index, try!(String::read_from(buf))),
                4 => m.put_raw(index, try!(format::Component::read_from(buf))),
                5 => m.put_raw(index, try!(Option::<item::Stack>::read_from(buf))),
                6 => m.put_raw(index, try!(bool::read_from(buf))),
                7 => m.put_raw(index, [
                    try!(f32::read_from(buf)),
                    try!(f32::read_from(buf)),
                    try!(f32::read_from(buf))
                ]),
                8 => m.put_raw(index, try!(super::Position::read_from(buf))),
                9 => {
                    if try!(bool::read_from(buf)) {
                        m.put_raw(index, try!(Option::<super::Position>::read_from(buf)));
                    } else {
                        m.put_raw::<Option<super::Position>>(index, None);
                    }
                },
                10 => m.put_raw(index, try!(protocol::VarInt::read_from(buf))),
                11 => {
                    if try!(bool::read_from(buf)) {
                        m.put_raw(index, try!(Option::<protocol::UUID>::read_from(buf)));
                    } else {
                        m.put_raw::<Option<protocol::UUID>>(index, None);
                    }
                },
                12 => m.put_raw(index, try!(protocol::VarInt::read_from(buf)).0 as u16),
                _ => return Err(io::Error::new(io::ErrorKind::InvalidInput, protocol::Error::Err("unknown metadata type".to_owned()))),
            }
        }
        Ok(m)
    }

    fn write_to(&self, buf: &mut io::Write) -> Result<(), io::Error> {
        for (k, v) in &self.map {
            try!((*k as u8).write_to(buf));
            match *v {
                Value::Byte(ref val) => {
                    try!(u8::write_to(&0, buf));
                    try!(val.write_to(buf));
                },
                Value::Int(ref val) => {
                    try!(u8::write_to(&1, buf));
                    try!(protocol::VarInt(*val).write_to(buf));
                },
                Value::Float(ref val) => {
                    try!(u8::write_to(&2, buf));
                    try!(val.write_to(buf));
                },
                Value::String(ref val) => {
                    try!(u8::write_to(&3, buf));
                    try!(val.write_to(buf));
                },
                Value::FormatComponent(ref val) => {
                    try!(u8::write_to(&4, buf));
                    try!(val.write_to(buf));
                },
                Value::OptionalItemStack(ref val) => {
                    try!(u8::write_to(&5, buf));
                    try!(val.write_to(buf));
                },
                Value::Bool(ref val) => {
                    try!(u8::write_to(&6, buf));
                    try!(val.write_to(buf));
                },
                Value::Vector(ref val) => {
                    try!(u8::write_to(&7, buf));
                    try!(val[0].write_to(buf));
                    try!(val[1].write_to(buf));
                    try!(val[2].write_to(buf));
                },
                Value::Position(ref val) => {
                    try!(u8::write_to(&8, buf));
                    try!(val.write_to(buf));
                },
                Value::OptionalPosition(ref val) => {
                    try!(u8::write_to(&9, buf));
                    try!(val.is_some().write_to(buf));
                    try!(val.write_to(buf));
                },
                Value::Direction(ref val) => {
                    try!(u8::write_to(&10, buf));
                    try!(val.write_to(buf));
                }
                Value::OptionalUUID(ref val) => {
                    try!(u8::write_to(&11, buf));
                    try!(val.is_some().write_to(buf));
                    try!(val.write_to(buf));
                },
                Value::Block(ref val) => {
                    try!(u8::write_to(&11, buf));
                    try!(protocol::VarInt(*val as i32).write_to(buf));
                }
            }
        }
        try!(u8::write_to(&0xFF, buf));
        Ok(())
    }
}

impl fmt::Debug for Metadata {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        try!(write!(f, "Metadata[ "));
        for (k, v) in &self.map {
            try!(write!(f, "{:?}={:?}, ", k, v));
        }
        write!(f, "]")
    }
}

impl Default for Metadata {
    fn default() -> Metadata {
        Metadata::new()
    }
}

#[derive(Debug)]
enum Value {
    Byte(i8),
    Int(i32),
    Float(f32),
    String(String),
    FormatComponent(format::Component),
    OptionalItemStack(Option<item::Stack>),
    Bool(bool),
    Vector([f32; 3]),
    Position(super::Position),
    OptionalPosition(Option<super::Position>),
    Direction(protocol::VarInt), // TODO: Proper type
    OptionalUUID(Option<protocol::UUID>),
    Block(u16), // TODO: Proper type
}

pub trait MetaValue {
    fn unwrap(&Value) -> &Self;
    fn wrap(self) -> Value;
}

impl MetaValue for i8 {
    fn unwrap(value: &Value) -> &Self {
        match *value {
            Value::Byte(ref val) => val,
            _ => panic!("incorrect key"),
        }
    }
    fn wrap(self) -> Value {
        Value::Byte(self)
    }
}

impl MetaValue for i32 {
    fn unwrap(value: &Value) -> &Self {
        match *value {
            Value::Int(ref val) => val,
            _ => panic!("incorrect key"),
        }
    }
    fn wrap(self) -> Value {
        Value::Int(self)
    }
}

impl MetaValue for f32 {
    fn unwrap(value: &Value) -> &Self {
        match *value {
            Value::Float(ref val) => val,
            _ => panic!("incorrect key"),
        }
    }
    fn wrap(self) -> Value {
        Value::Float(self)
    }
}

impl MetaValue for String {
    fn unwrap(value: &Value) -> &Self {
        match *value {
            Value::String(ref val) => val,
            _ => panic!("incorrect key"),
        }
    }
    fn wrap(self) -> Value {
        Value::String(self)
    }
}

impl MetaValue for format::Component {
    fn unwrap(value: &Value) -> &Self {
        match *value {
            Value::FormatComponent(ref val) => val,
            _ => panic!("incorrect key"),
        }
    }
    fn wrap(self) -> Value {
        Value::FormatComponent(self)
    }
}

impl MetaValue for Option<item::Stack> {
    fn unwrap(value: &Value) -> &Self {
        match *value {
            Value::OptionalItemStack(ref val) => val,
            _ => panic!("incorrect key"),
        }
    }
    fn wrap(self) -> Value {
        Value::OptionalItemStack(self)
    }
}

impl MetaValue for bool {
    fn unwrap(value: &Value) -> &Self {
        match *value {
            Value::Bool(ref val) => val,
            _ => panic!("incorrect key"),
        }
    }
    fn wrap(self) -> Value {
        Value::Bool(self)
    }
}

impl MetaValue for [f32; 3] {
    fn unwrap(value: &Value) -> &Self {
        match *value {
            Value::Vector(ref val) => val,
            _ => panic!("incorrect key"),
        }
    }
    fn wrap(self) -> Value {
        Value::Vector(self)
    }
}

impl MetaValue for super::Position {
    fn unwrap(value: &Value) -> &Self {
        match *value {
            Value::Position(ref val) => val,
            _ => panic!("incorrect key"),
        }
    }
    fn wrap(self) -> Value {
        Value::Position(self)
    }
}

impl MetaValue for Option<super::Position> {
    fn unwrap(value: &Value) -> &Self {
        match *value {
            Value::OptionalPosition(ref val) => val,
            _ => panic!("incorrect key"),
        }
    }
    fn wrap(self) -> Value {
        Value::OptionalPosition(self)
    }
}

impl MetaValue for protocol::VarInt {
    fn unwrap(value: &Value) -> &Self {
        match *value {
            Value::Direction(ref val) => val,
            _ => panic!("incorrect key"),
        }
    }
    fn wrap(self) -> Value {
        Value::Direction(self)
    }
}

impl MetaValue for Option<protocol::UUID> {
    fn unwrap(value: &Value) -> &Self {
        match *value {
            Value::OptionalUUID(ref val) => val,
            _ => panic!("incorrect key"),
        }
    }
    fn wrap(self) -> Value {
        Value::OptionalUUID(self)
    }
}

impl MetaValue for u16 {
    fn unwrap(value: &Value) -> &Self {
        match *value {
            Value::Block(ref val) => val,
            _ => panic!("incorrect key"),
        }
    }
    fn wrap(self) -> Value {
        Value::Block(self)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::marker::PhantomData;

    const TEST: MetadataKey<String> =
        MetadataKey {
            index: 0,
            ty: PhantomData,
        };

    #[test]
    fn basic() {
        let mut m = Metadata::new();

        m.put(&TEST, "Hello world".to_owned());

        match m.get(&TEST) {
            Some(val) => {
                assert!(val == "Hello world");
            }
            None => panic!("failed"),
        }
    }
}