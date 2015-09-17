#![allow(dead_code)]

use openssl;
use serde_json;

pub mod mojang;

use nbt;
use format;
use std::fmt;
use std::default;
use std::net::TcpStream;
use std::io;
use std::io::{Write, Read};
use std::convert;
use byteorder::{BigEndian, WriteBytesExt, ReadBytesExt};
use flate2::read::{ZlibDecoder, ZlibEncoder};
use flate2;

/// Helper macro for defining packets
#[macro_export]
macro_rules! state_packets {
     ($($state:ident $stateName:ident {
        $($dir:ident $dirName:ident {
            $(
                $name:ident => $id:expr {
                $($field:ident: $field_type:ty = $(when ($cond:expr))*, )+
            })*
        })+
    })+) => {
        use protocol::*;
        use std::io;

        #[derive(Debug)]
        pub enum Packet {
        $(
            $(
                $(
        $name($state::$dir::$name),
                )*
            )+
        )+
        }

        $(
        pub mod $state {
            $(
            pub mod $dir {
                #![allow(unused_imports)]
                use protocol::*;
                use std::io;
                use format;
                use nbt;
                use types;
                use item;

                $(
                    #[derive(Default, Debug)]
                    pub struct $name {
                        $(pub $field: $field_type),+,
                    }

                    impl PacketType for $name {

                        fn packet_id(&self) -> i32{ $id }

                        fn write(self, buf: &mut io::Write) -> Result<(), io::Error> {
                            $(
                                if true $(&& ($cond(&self)))* {
                                    try!(self.$field.write_to(buf));
                                }
                            )+

                            Result::Ok(())
                        }
                    }
                )*
            }
            )+
        }
        )+

        /// Returns the packet for the given state, direction and id after parsing the fields
        /// from the buffer.
        pub fn packet_by_id(state: State, dir: Direction, id: i32, mut buf: &mut io::Read) -> Result<Option<Packet>, io::Error> {
            match state {
                $(
                    State::$stateName => {
                        match dir {
                            $(
                                Direction::$dirName => {
                                    match id {
                                    $(
                                        $id => {
                                            use self::$state::$dir::$name;
                                            let mut packet : $name = $name::default();
                                            $(
                                                if true $(&& ($cond(&packet)))* {
                                                    packet.$field = try!(Serializable::read_from(&mut buf));
                                                }
                                            )+
                                            Result::Ok(Option::Some(Packet::$name(packet)))
                                        },
                                    )*
                                        _ => Result::Ok(Option::None)
                                    }
                                }
                            )+
                        }
                    }
                )+
            }
        }
    }
}

pub mod packet;

pub trait Serializable {
    fn read_from(buf: &mut io::Read) -> Result<Self, io::Error>;
    fn write_to(&self, buf: &mut io::Write) -> Result<(), io::Error>;
}

impl Serializable for Vec<u8> {
    fn read_from(buf: &mut io::Read) -> Result<Vec<u8> , io::Error> {
        let mut v = Vec::new();
        try!(buf.read_to_end(&mut v));
        Ok(v)
    }

    fn write_to(&self, buf: &mut io::Write) -> Result<(), io::Error> {
        buf.write_all(&self[..])
    }
}

impl Serializable for Option<nbt::NamedTag>{
    fn read_from(buf: &mut io::Read) -> Result<Option<nbt::NamedTag>, io::Error> {
        let ty = try!(buf.read_u8());
        if ty == 0 {
            Result::Ok(None)
        } else {
            let name = try!(nbt::read_string(buf));
            let tag = try!(nbt::Tag::read_from(buf));
            Result::Ok(Some(nbt::NamedTag(name, tag)))
        }
    }
    fn write_to(&self, buf: &mut io::Write) -> Result<(), io::Error> {
        match *self {
            Some(ref val) => {
                try!(buf.write_u8(10));
                try!(nbt::write_string(buf, &val.0));
                try!(val.1.write_to(buf));
            }
            None => try!(buf.write_u8(0)),
        }
        Result::Ok(())
    }
}

impl <T> Serializable for Option<T> where T : Serializable {
    fn read_from(buf: &mut io::Read) -> Result<Option<T>, io::Error> {
        Result::Ok(Some(try!(T::read_from(buf))))
    }
    fn write_to(&self, buf: &mut io::Write) -> Result<(), io::Error> {
        if self.is_some() {
            try!(self.as_ref().unwrap().write_to(buf));
        }
        Result::Ok(())
    }
}

impl Serializable for String {
    fn read_from(buf: &mut io::Read) -> Result<String, io::Error> {
        let len = try!(VarInt::read_from(buf)).0;
        let mut ret = String::new();
        try!(buf.take(len as u64).read_to_string(&mut ret));
        Result::Ok(ret)
    }
    fn write_to(&self, buf: &mut io::Write) -> Result<(), io::Error> {
        let bytes = self.as_bytes();
        try!(VarInt(bytes.len() as i32).write_to(buf));
        try!(buf.write_all(bytes));
        Result::Ok(())
    }
}

impl Serializable for format::Component {
    fn read_from(buf: &mut io::Read) -> Result<Self, io::Error> {
        let len = try!(VarInt::read_from(buf)).0;
        let mut ret = String::new();
        try!(buf.take(len as u64).read_to_string(&mut ret));
        let val : serde_json::Value = serde_json::from_str(&ret[..]).unwrap();
        Result::Ok(Self::from_value(&val))
    }
    fn write_to(&self, buf: &mut io::Write) -> Result<(), io::Error> {
        let val = serde_json::to_string(&self.to_value()).unwrap();
        let bytes = val.as_bytes();
        try!(VarInt(bytes.len() as i32).write_to(buf));
        try!(buf.write_all(bytes));
        Result::Ok(())
    }
}

impl Serializable for () {
    fn read_from(_: &mut io::Read) -> Result<(), io::Error> {
        Result::Ok(())
    }
    fn write_to(&self, _: &mut io::Write) -> Result<(), io::Error> {
        Result::Ok(())
    }
}

impl Serializable for bool {
    fn read_from(buf: &mut io::Read) -> Result<bool, io::Error> {
        Result::Ok(try!(buf.read_u8()) != 0)
    }
    fn write_to(&self, buf: &mut io::Write) -> Result<(), io::Error> {
        try!(buf.write_u8(if *self { 1 } else { 0 }));
        Result::Ok(())
    }
}

impl Serializable for i8 {
    fn read_from(buf: &mut io::Read) -> Result<i8, io::Error> {
        Result::Ok(try!(buf.read_i8()))
    }
    fn write_to(&self, buf: &mut io::Write) -> Result<(), io::Error> {
        try!(buf.write_i8(*self));
        Result::Ok(())
    }
}

impl Serializable for i16 {
    fn read_from(buf: &mut io::Read) -> Result<i16, io::Error> {
        Result::Ok(try!(buf.read_i16::<BigEndian>()))
    }
    fn write_to(&self, buf: &mut io::Write) -> Result<(), io::Error> {
        try!(buf.write_i16::<BigEndian>(*self));
        Result::Ok(())
    }
}

impl Serializable for i32 {
    fn read_from(buf: &mut io::Read) -> Result<i32, io::Error> {
        Result::Ok(try!(buf.read_i32::<BigEndian>()))
    }
    fn write_to(&self, buf: &mut io::Write) -> Result<(), io::Error> {
        try!(buf.write_i32::<BigEndian>(*self));
        Result::Ok(())
    }
}

impl Serializable for i64 {
    fn read_from(buf: &mut io::Read) -> Result<i64, io::Error> {
        Result::Ok(try!(buf.read_i64::<BigEndian>()))
    }
    fn write_to(&self, buf: &mut io::Write) -> Result<(), io::Error> {
        try!(buf.write_i64::<BigEndian>(*self));
        Result::Ok(())
    }
}

impl Serializable for u8 {
    fn read_from(buf: &mut io::Read) -> Result<u8, io::Error> {
        Result::Ok(try!(buf.read_u8()))
    }
    fn write_to(&self, buf: &mut io::Write) -> Result<(), io::Error> {
        try!(buf.write_u8(*self));
        Result::Ok(())
    }
}

impl Serializable for u16 {
    fn read_from(buf: &mut io::Read) -> Result<u16, io::Error> {
        Result::Ok(try!(buf.read_u16::<BigEndian>()))
    }
    fn write_to(&self, buf: &mut io::Write) -> Result<(), io::Error> {
        try!(buf.write_u16::<BigEndian>(*self));
        Result::Ok(())
    }
}

impl Serializable for f32 {
    fn read_from(buf: &mut io::Read) -> Result<f32, io::Error> {
        Result::Ok(try!(buf.read_f32::<BigEndian>()))
    }
    fn write_to(&self, buf: &mut io::Write) -> Result<(), io::Error> {
        try!(buf.write_f32::<BigEndian>(*self));
        Result::Ok(())
    }
}

impl Serializable for f64 {
    fn read_from(buf: &mut io::Read) -> Result<f64, io::Error> {
        Result::Ok(try!(buf.read_f64::<BigEndian>()))
    }
    fn write_to(&self, buf: &mut io::Write) -> Result<(), io::Error> {
        try!(buf.write_f64::<BigEndian>(*self));
        Result::Ok(())
    }
}

#[derive(Debug)]
pub struct UUID(u64, u64);

impl Default for UUID {
    fn default() -> Self { UUID(0, 0) }
}

impl Serializable for UUID {
    fn read_from(buf: &mut io::Read) -> Result<UUID, io::Error> {
        Result::Ok(
            UUID(
                try!(buf.read_u64::<BigEndian>()),
                try!(buf.read_u64::<BigEndian>()),
            )
        )
    }
    fn write_to(&self, buf: &mut io::Write) -> Result<(), io::Error> {
        try!(buf.write_u64::<BigEndian>(self.0));
        try!(buf.write_u64::<BigEndian>(self.1));
        Result::Ok(())
    }
}


pub trait Lengthable : Serializable + Copy + Default {
    fn into(self) -> usize;
    fn from(usize) -> Self;
}

pub struct LenPrefixed<L: Lengthable, V> {
    len: L,
    pub data: Vec<V>
}

impl <L: Lengthable, V: Default>  LenPrefixed<L, V> {
    fn new(data: Vec<V>) -> LenPrefixed<L, V> {
        return LenPrefixed {
            len: Default::default(),
            data: data,
        }
    }
}

impl <L: Lengthable, V: Serializable>  Serializable for LenPrefixed<L, V> {
    fn read_from(buf: &mut io::Read) -> Result<LenPrefixed<L, V>, io::Error> {
        let len_data : L = try!(Serializable::read_from(buf));
        let len : usize = len_data.into();
        let mut data : Vec<V> = Vec::with_capacity(len);
        for _ in 0 .. len {
            data.push(try!(Serializable::read_from(buf)));
        }
        Result::Ok(LenPrefixed{len: len_data, data: data})
    }

    fn write_to(&self, buf: &mut io::Write) -> Result<(), io::Error> {
        let len_data : L = L::from(self.data.len());
        try!(len_data.write_to(buf));
        let ref data = self.data;
        for val in data {
            try!(val.write_to(buf));
        }
        Result::Ok(())
    }
}


impl <L: Lengthable, V: Default> Default for LenPrefixed<L, V> {
    fn default() -> Self {
        LenPrefixed {
            len: default::Default::default(),
            data: default::Default::default()
        }
    }
}

impl <L: Lengthable, V: fmt::Debug> fmt::Debug for LenPrefixed<L, V> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.data.fmt(f)
    }
}

// Optimization
pub struct LenPrefixedBytes<L: Lengthable> {
    len: L,
    pub data: Vec<u8>
}

impl <L: Lengthable>  LenPrefixedBytes<L> {
    fn new(data: Vec<u8>) -> LenPrefixedBytes<L> {
        return LenPrefixedBytes {
            len: Default::default(),
            data: data,
        }
    }
}

impl <L: Lengthable>  Serializable for LenPrefixedBytes<L> {
    fn read_from(buf: &mut io::Read) -> Result<LenPrefixedBytes<L>, io::Error> {
        let len_data : L = try!(Serializable::read_from(buf));
        let len : usize = len_data.into();
        let mut data : Vec<u8> = Vec::with_capacity(len);
        try!(buf.take(len as u64).read_to_end(&mut data));
        Result::Ok(LenPrefixedBytes{len: len_data, data: data})
    }

    fn write_to(&self, buf: &mut io::Write) -> Result<(), io::Error> {
        let len_data : L = L::from(self.data.len());
        try!(len_data.write_to(buf));
        try!(buf.write_all(&self.data[..]));
        Result::Ok(())
    }
}


impl <L: Lengthable> Default for LenPrefixedBytes<L> {
    fn default() -> Self {
        LenPrefixedBytes {
            len: default::Default::default(),
            data: default::Default::default()
        }
    }
}

impl <L: Lengthable> fmt::Debug for LenPrefixedBytes<L> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.data.fmt(f)
    }
}

impl Lengthable for i16 {
    fn into(self) -> usize {
        self as usize
    }

    fn from(u: usize) -> i16 {
        u as i16
    }
}

impl Lengthable for i32 {
    fn into(self) -> usize {
        self as usize
    }

    fn from(u: usize) -> i32 {
        u as i32
    }
}

/// VarInt have a variable size (between 1 and 5 bytes) when encoded based
/// on the size of the number
#[derive(Clone, Copy)]
pub struct VarInt(pub i32);

impl Lengthable for VarInt {
    fn into(self) -> usize {
        self.0 as usize
    }

    fn from(u: usize) -> VarInt {
        VarInt(u as i32)
    }
}

impl Serializable for VarInt {
    /// Decodes a VarInt from the Reader
    fn read_from(buf: &mut io::Read) -> Result<VarInt, io::Error> {
        const PART : u32 = 0x7F;
        let mut size = 0;
        let mut val = 0u32;
        loop {
            let b = try!(buf.read_u8()) as u32;
            val |= (b & PART) << (size * 7);
            size+=1;
            if size > 5 {
                return Result::Err(io::Error::new(io::ErrorKind::InvalidInput, Error::Err("VarInt too big".to_owned())))
            }
            if (b & 0x80) == 0 {
                break
            }
        }

        Result::Ok(VarInt(val as i32))
    }

    /// Encodes a VarInt into the Writer
    fn write_to(&self, buf: &mut io::Write) -> Result<(), io::Error> {
        const PART : u32 = 0x7F;
        let mut val = self.0 as u32;
        loop {
            if (val & !PART) == 0 {
                try!(buf.write_u8(val as u8));
                return Result::Ok(());
            }
            try!(buf.write_u8(((val & PART) | 0x80) as u8));
            val >>= 7;
        }
    }
}

impl default::Default for VarInt {
    fn default() -> VarInt { VarInt(0) }
}

impl fmt::Debug for VarInt {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// VarLong have a variable size (between 1 and 10 bytes) when encoded based
/// on the size of the number
#[derive(Clone, Copy)]
pub struct VarLong(pub i64);

impl Lengthable for VarLong {
    fn into(self) -> usize {
        self.0 as usize
    }

    fn from(u: usize) -> VarLong {
        VarLong(u as i64)
    }
}

impl Serializable for VarLong {
    /// Decodes a VarLong from the Reader
    fn read_from(buf: &mut io::Read) -> Result<VarLong, io::Error> {
        const PART : u64 = 0x7F;
        let mut size = 0;
        let mut val = 0u64;
        loop {
            let b = try!(buf.read_u8()) as u64;
            val |= (b & PART) << (size * 7);
            size+=1;
            if size > 10 {
                return Result::Err(io::Error::new(io::ErrorKind::InvalidInput, Error::Err("VarLong too big".to_owned())))
            }
            if (b & 0x80) == 0 {
                break
            }
        }

        Result::Ok(VarLong(val as i64))
    }

    /// Encodes a VarLong into the Writer
    fn write_to(&self, buf: &mut io::Write) -> Result<(), io::Error> {
        const PART : u64 = 0x7F;
        let mut val = self.0 as u64;
        loop {
            if (val & !PART) == 0 {
                try!(buf.write_u8(val as u8));
                return Result::Ok(());
            }
            try!(buf.write_u8(((val & PART) | 0x80) as u8));
            val >>= 7;
        }
    }
}

impl default::Default for VarLong {
    fn default() -> VarLong { VarLong(0) }
}

impl fmt::Debug for VarLong {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Direction is used to define whether packets are going to the
/// server or the client.
#[derive(Clone, Copy)]
pub enum Direction {
    Serverbound,
    Clientbound
}

/// The protocol has multiple 'sub-protocols' or states which control which
/// packet an id points to.
#[derive(Clone, Copy)]
pub enum State {
    Handshaking,
    Play,
    Status,
    Login
}

/// Return for any protocol related error.
#[derive(Debug)]
pub enum Error {
    Err(String),
    IOError(io::Error),
}

impl convert::From<io::Error> for Error {
    fn from(e : io::Error) -> Error {
        Error::IOError(e)
    }
}

impl ::std::error::Error for Error {
    fn description(&self) -> &str {
        match self {
            &Error::Err(ref val) => &val[..],
            &Error::IOError(ref e) => e.description(),
        }
    }
}

impl ::std::fmt::Display for Error {
    fn fmt(&self, f : &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        match self {
            &Error::Err(ref val) => write!(f, "protocol error: {}", val),
            &Error::IOError(ref e) => e.fmt(f)
        }
    }
}

pub struct Conn {
    stream: TcpStream,
    host: String,
    port: u16,
    direction: Direction,
    state: State,

    cipher: Option<openssl::EVPCipher>,

    compression_threshold: i32,
    compression_read: Option<ZlibDecoder<io::Cursor<Vec<u8>>>>, // Pending reset support
    compression_write: Option<ZlibEncoder<io::Cursor<Vec<u8>>>>,
}

impl Conn {
    pub fn new(target: &str) -> Result<Conn, Error>{
        // TODO SRV record support
        let stream = match TcpStream::connect(target) {
            Ok(val) => val,
            Err(err) => return Result::Err(Error::IOError(err))
        };
        let parts = target.split(":").collect::<Vec<&str>>();
        Result::Ok(Conn {
            stream: stream,
            host: parts[0].to_owned(),
            port: parts[1].parse().unwrap(),
            direction: Direction::Serverbound,
            state: State::Handshaking,
            cipher: Option::None,
            compression_threshold: -1,
            compression_read: Option::None,
            compression_write: Option::None,
        })
    }

    pub fn write_packet<T: PacketType>(&mut self, packet: T) -> Result<(), Error> {
        let mut buf = Vec::new();
        try!(VarInt(packet.packet_id()).write_to(&mut buf));
        try!(packet.write(&mut buf));

        let mut extra = if self.compression_threshold >= 0 { 1 } else { 0 };
        if self.compression_threshold >= 0 && buf.len() as i32 > self.compression_threshold {
            extra = 0;
            let uncompressed_size = buf.len();
            let mut new = Vec::new();
            try!(VarInt(uncompressed_size as i32).write_to(&mut new));
            let mut write = self.compression_write.as_mut().unwrap();
            write.reset(io::Cursor::new(buf));
            try!(write.read_to_end(&mut new));
            buf = new;
        }

        try!(VarInt(buf.len() as i32 + extra).write_to(self));
        if self.compression_threshold >= 0 && extra == 1 {
            try!(VarInt(0).write_to(self));
        }
        try!(self.write_all(&buf.into_boxed_slice()));

        Result::Ok(())
    }

    pub fn read_packet(&mut self) -> Result<packet::Packet, Error> {
        let len = try!(VarInt::read_from(self)).0 as usize;
        let mut ibuf = Vec::with_capacity(len);
        try!(self.take(len as u64).read_to_end(&mut ibuf));

        let mut buf = io::Cursor::new(ibuf);

        if self.compression_threshold >= 0 {
            let uncompressed_size = try!(VarInt::read_from(&mut buf)).0;
            if uncompressed_size != 0 {
                let mut new = Vec::with_capacity(uncompressed_size as usize);
                {
                    let mut reader = self.compression_read.as_mut().unwrap();
                    reader.reset(buf);
                    try!(reader.read_to_end(&mut new));
                }
                buf = io::Cursor::new(new);
            }
        }
        let id = try!(VarInt::read_from(&mut buf)).0;

        let dir = match self.direction {
            Direction::Clientbound => Direction::Serverbound,
            Direction::Serverbound => Direction::Clientbound,
        };

        let packet = try!(packet::packet_by_id(self.state, dir, id, &mut buf));

        match packet {
            Some(val) => {
                let pos = buf.position() as usize;
                let ibuf = buf.into_inner();
                if ibuf.len() != pos {
                    return Result::Err(Error::Err(format!("Failed to read all of packet 0x{:X}, had {} bytes left", id, ibuf.len() - pos)))
                }
                Result::Ok(val)
            },
            None => Result::Err(Error::Err("missing packet".to_owned()))
        }
    }

    pub fn enable_encyption(&mut self, key: &Vec<u8>, decrypt: bool) {
        self.cipher = Option::Some(openssl::EVPCipher::new(key, key, decrypt));
    }

    pub fn set_compresssion(&mut self, threshold: i32, read: bool) {
        self.compression_threshold = threshold;
        if !read {
            self.compression_write = Some(ZlibEncoder::new(io::Cursor::new(Vec::new()), flate2::Compression::Default));
        } else {
            self.compression_read = Some(ZlibDecoder::new(io::Cursor::new(Vec::new())));
        }
    }
}

impl Read for Conn {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self.cipher.as_mut() {
            Option::None => self.stream.read(buf),
            Option::Some(cipher) => {
                let ret = try!(self.stream.read(buf));
                let data = cipher.decrypt(&buf[..ret]);
                for i in 0 .. ret {
                    buf[i] = data[i];
                }
                Ok(ret)
            },
        }
    }
}

impl Write for Conn {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self.cipher.as_mut() {
            Option::None => self.stream.write(buf),
            Option::Some(cipher) => {
                let data = cipher.encrypt(buf);
                try!(self.stream.write_all(&data[..]));
                Ok(buf.len())
            },
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}

impl Clone for Conn {
    fn clone(&self) -> Self {
        Conn {
            stream: self.stream.try_clone().unwrap(),
            host: self.host.clone(),
            port: self.port,
            direction: self.direction,
            state: self.state,
            cipher: Option::None,
            compression_threshold: self.compression_threshold,
            compression_read: Option::None,
            compression_write: Option::None,
        }
    }
}

pub trait PacketType {
    fn packet_id(&self) -> i32;

    fn write(self, buf: &mut io::Write) -> Result<(), io::Error>;
}

// #[test]
pub fn test() {
    let mut c = Conn::new("localhost:25565").unwrap();

    c.write_packet(packet::handshake::serverbound::Handshake{
        protocol_version: VarInt(71),
        host: "localhost".to_owned(),
        port: 25565,
        next: VarInt(2),
    }).unwrap();
    c.state = State::Login;
    c.write_packet(packet::login::serverbound::LoginStart{username: "Think".to_owned()}).unwrap();

    let packet = match c.read_packet().unwrap() {
        packet::Packet::EncryptionRequest(val) => val,
        val => panic!("Wrong packet: {:?}", val),
    };

    let mut key = openssl::PublicKey::new(&packet.public_key.data);
    let shared = openssl::gen_random(16);

    let shared_e = key.encrypt(&shared);
    let token_e = key.encrypt(&packet.verify_token.data);

    let profile = mojang::Profile{
        username: "Think".to_owned(),
        id: "b1184d43168441cfa2128b9a3df3b6ab".to_owned(),
        access_token: "".to_owned()
    };

    profile.join_server(&packet.server_id, &shared, &packet.public_key.data);

    c.write_packet(packet::login::serverbound::EncryptionResponse{
        shared_secret: LenPrefixedBytes::new(shared_e),
        verify_token: LenPrefixedBytes::new(token_e),
    }).unwrap();

    let mut read = c.clone();
    let mut write = c.clone();

    read.enable_encyption(&shared, true);
    write.enable_encyption(&shared, false);

    loop { match read.read_packet().unwrap() {
        packet::Packet::LoginDisconnect(val) => {
            panic!("Discconect {}", val.reason);
        },
        packet::Packet::SetInitialCompression(val) => {
            read.set_compresssion(val.threshold.0, true);
            write.set_compresssion(val.threshold.0, false);
            println!("Compression: {}", val.threshold.0)
        },
        packet::Packet::LoginSuccess(val) => {
            println!("Login: {} {}", val.username, val.uuid);
            read.state = State::Play;
            write.state = State::Play;
            break;
        }
        _ => panic!("Unknown packet"),
    } }

    let mut first = true;
    let mut count = 0;
    loop { match read.read_packet().unwrap() {
            packet::Packet::ServerMessage(val) => println!("MSG: {}", val.message),
            packet::Packet::ChunkData(_) => {},
            val => {
                println!("{:?}", val);
                if first {
                    write.write_packet(packet::play::serverbound::ChatMessage{
                        message: "Hello world".to_owned(),
                    }).unwrap();
                    first = false;
                }
                count += 1;
                if count > 200 {
                    break;
                }
            }
    } }

    unimplemented!();
}