//! public interface types

use std::{cmp::Ordering, fmt::Display, hint::unreachable_unchecked, mem};

pub(crate) const VM_CODE: usize = 0;
pub(crate) const VM_DATA: usize = 1;
pub(crate) const VM_INVAR: usize = 2;
pub(crate) const VM_STACK: usize = 3;

#[non_exhaustive]
#[derive(Debug, Clone, Copy)]
pub enum VMErrorClass {
    Other,
    NotImpl,
    NotSupp,
    Invalid,
    Perms,
    Boundary,
    ETodo,
    Default,
    Halt,
}
impl VMErrorClass {
    pub fn canon_name(&self) -> &'static str {
        match self {
            Self::Other => "Misc",
            Self::NotImpl => "NotImplemented",
            Self::NotSupp => "NotSupported",
            Self::Invalid => "Invalid",
            Self::Perms => "PermissionViolation",
            Self::Boundary => "BoundaryViolation",
            Self::ETodo => "Unset Error Type",
            Self::Default => "DefaultError",
            Self::Halt => "Halted",
        }
    }
}

#[derive(Debug, Clone)]
pub struct VMError {
    class: VMErrorClass,
    msg: Option<String>,
}
impl Default for VMError {
    fn default() -> Self {
        Self { class: VMErrorClass::Other, msg: None }
    }
}

#[macro_export]
macro_rules! etodo {
    () => {
        VMError::from_owned(VMErrorClass::ETodo, format!("SRC {}:{}:{}", file!(), line!(), column!()))
    };
}
#[macro_export]
macro_rules! vmerror {
    () => {
        VMError::DEFAULT
    };
    ("todo") => {vmerror!("todo", "unspecified")};
    ($msg:literal) => {
        VMError::from_msg($msg)
    };
    ($msg:expr) => {
        VMError::from_owned(VMErrorClass::Other, $msg)
    };
    ("todo", $msg:literal) => {
        VMError::from_owned(VMErrorClass::NotImpl, format!("todo feature: {}", $msg))
    };
}

impl VMError {
    pub fn from_owned(class: VMErrorClass, msg: String) -> Self {
        Self { class, msg: Some(msg) }
    }
    pub fn new(class: VMErrorClass, msg: &str) -> Self {
        Self { class, msg: Some(msg.to_owned()) }
    }
    pub fn from_msg(msg: &str) -> Self {
        Self {
            class: VMErrorClass::Other,
            msg: Some(msg.to_owned()),
        }
    }
    pub fn from_class(class: VMErrorClass) -> Self {
        Self { class, msg: None }
    }
}
impl Display for VMError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "VMError ({})", self.class.canon_name())?;
        if let Some(msg) = self.msg.as_ref() {
            write!(f, " {}", msg)
        } else {
            write!(f, " {}", "{no message provided}")
        }
    }
}

impl std::error::Error for VMError {}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VMPurpose {
    T3R = 0,
    GPC,
    BOT,
    SBC,
    UKN,
}
impl VMPurpose {
    pub fn from_byte(b: u8) -> Self {
        match b {
            0 => Self::T3R,
            1 => Self::GPC,
            2 => Self::BOT,
            3 => Self::SBC,
            _ => Self::UKN,
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum TStructDescriptor<'a> {
    Root(&'a TStructDescriptor<'a>),
    Invalid,
}

impl TStructDescriptor<'static> {
    pub(crate) const UNINIT: TStructDescriptor<'static> = TStructDescriptor::Root(&Self::Invalid);
}

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub enum VMType<'a> {
    U8,
    S8,
    U16,
    S16,
    U32,
    S32,
    U64,
    S64,
    U128,
    S128,
    F32,
    F64,
    SSTR,
    LSTR,
    SARR(u8, Box<VMType<'a>>),
    UARR(Box<VMType<'a>>),
    PTR(Box<VMType<'a>>),
    VOID,
    OSTRUCT,
    TSTRUCT(&'a TStructDescriptor<'a>),
    INVALID,
}

impl<'a> VMType<'a> {
    pub fn sizeof(&self) -> Option<usize> {
        Some(match self {
            Self::U8|Self::S8 => 1,
            Self::U16|Self::S16 => 2,
            Self::U32|Self::S32|Self::PTR(_) => 4,
            Self::U64|Self::S64 => 8,
            Self::F32 => 4,
            Self::F64 => 8,
            _ => {return None;}
        })
    }
    pub fn signed(&self) -> bool {
        match self {
            Self::S8|Self::S16|Self::S32|Self::S64|Self::S128=>true,
            _ => false,
        }
    }
    pub fn to_unsigned(&self) -> Option<Self> {
        Some(match self {
            Self::U8|Self::U16|Self::U32|Self::U64|Self::U128 => self.clone(),
            Self::S8 => Self::U8,
            Self::S16 => Self::U16,
            Self::S32 => Self::U32,
            Self::S64 => Self::U64,
            Self::S128 => Self::U128,
            _ => {return None;}
        })
    }
    pub fn to_signed(&self) -> Option<Self> {
        Some(match self {
            Self::S8|Self::S16|Self::S32|Self::S64|Self::S128 => self.clone(),
            Self::U8 => Self::S8,
            Self::U16 => Self::S16,
            Self::U32 => Self::S32,
            Self::U64 => Self::S64,
            Self::U128 => Self::S128,
            _ => {return None;}
        })
    }
}

impl Display for VMType<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::INVALID => "!",
            Self::VOID => "void",
            Self::PTR(p) => {return f.write_str(&format!("{}*", p));}
            Self::SARR(s, p) => {return f.write_str(&format!("{}[{}]", p, s));}
            Self::UARR(p) => {return f.write_str(&format!("{}[]", p));}
            Self::TSTRUCT(..) => unimplemented!(),
            Self::OSTRUCT => "?",
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
            Self::U128 => "u128",
            Self::S8 => "s8",
            Self::S16 => "s16",
            Self::S32 => "s32",
            Self::S64 => "s64",
            Self::S128 => "s128",
            Self::F32 => "f32",
            Self::F64 => "f64",
            Self::SSTR => "sstr",
            Self::LSTR => "lstr",
        })
    }
}

#[derive(Debug, Clone)]
pub struct VMValue<'a>(pub VMType<'a>, pub Box<[u8]>);

pub type VMResult<T> = Result<T, VMError>;

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum Register {
    R0 = 0,
    R1,
    R2,
    R3,
    R4,
    R5,
    R6,
    R7,
    R8,
    R9,
    R10,
    R11,
    R12,
    R13,
    R14,
    R15,
    SP,
    BP,
    PC,
    CF,
    RF0,
    RF1,
    RF2,
    RF3,
    RF4,
    RF5,
    INVAR,
    DATA,

    INVALID
}

impl Register {
    /// one of R13,R14,R15,INVAR,DATA
    pub fn is_cvr(&self) -> bool {
        return (*self >= Self::R13 && *self <= Self::R15) || *self == Self::INVAR || *self == Self::DATA;
    }
    /// either CVR or one of SP,BP,PC,CF
    pub fn is_ror(&self) -> bool {
        return self.is_cvr() || (*self >= Self::SP && *self <= Self::CF);
    }
    pub fn is_fpr(&self) -> bool {
        return *self >= Self::RF0 && *self <= Self::RF5;
    }
    pub fn is_gpr(&self) -> bool {
        return *self <= Self::R12;
    }
    pub const fn count() -> usize {
        return Self::DATA as usize + 1;
    }
    pub fn from_byte(b: u8) -> Self {
        match b {
            0 => Self::R0,
            1 => Self::R1,
            2 => Self::R2,
            3 => Self::R3,
            4 => Self::R4,
            5 => Self::R5,
            6 => Self::R6,
            7 => Self::R7,
            8 => Self::R8,
            9 => Self::R9,
            10 => Self::R10,
            11 => Self::R11,
            12 => Self::R12,
            13 => Self::R13,
            14 => Self::R14,
            15 => Self::R15,
            16 => Self::SP,
            17 => Self::BP,
            18 => Self::PC,
            19 => Self::CF,
            20 => Self::RF0,
            21 => Self::RF1,
            22 => Self::RF2,
            23 => Self::RF3,
            24 => Self::RF4,
            25 => Self::RF5,
            26 => Self::INVAR,
            27 => Self::DATA,
            _ => Self::INVALID,
        }
    }
}

/// concrete value
/// 
/// this enum is used to represent fixed size values with unambiguous representations
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CValue {
    U8(u8),S8(i8),
    U16(u16),S16(i16),
    U32(u32),S32(i32),
    U64(u64),S64(i64),
    U128(u128),S128(i128),
    F32(f32),F64(f64),
}

impl CValue {
    pub fn sizeof(&self) -> usize {
        match self {
            Self::U8(_)|Self::S8(_) => 1,
            Self::U16(_)|Self::S16(_) => 2,
            Self::U32(_)|Self::S32(_) => 4,
            Self::U64(_)|Self::S64(_) => 8,
            Self::U128(_)|Self::S128(_) => 16,
            Self::F32(_) => 4,
            Self::F64(_) => 8,
        }
    }
    pub fn from_parts(t: VMType, data: &[u8]) -> Option<Self> {
        if data.len() != t.sizeof()? {
            // println!("size mismatch");
            return None;
        }
        Some(match t {
            VMType::U8 => Self::U8(data[0]),
            VMType::S8 => Self::S8(i8::from_be_bytes(data.try_into().unwrap())),
            VMType::U16 => Self::U16(u16::from_be_bytes(data.try_into().unwrap())),
            VMType::S16 => Self::S16(i16::from_be_bytes(data.try_into().unwrap())),
            VMType::PTR(..)|VMType::U32 => Self::U32(u32::from_be_bytes(data.try_into().unwrap())),
            VMType::S32 => Self::S32(i32::from_be_bytes(data.try_into().unwrap())),
            VMType::U64 => Self::U64(u64::from_be_bytes(data.try_into().unwrap())),
            VMType::S64 => Self::S64(i64::from_be_bytes(data.try_into().unwrap())),
            VMType::U128 => Self::U128(u128::from_be_bytes(data.try_into().unwrap())),
            VMType::S128 => Self::S128(i128::from_be_bytes(data.try_into().unwrap())),
            VMType::F32 => Self::F32(f32::from_be_bytes(data.try_into().unwrap())),
            VMType::F64 => Self::F64(f64::from_be_bytes(data.try_into().unwrap())),
            _ => {
                // println!("{t:?}");
                return None;
            }
        })
    }
    /// attempts to compose a [CValue] from a [VMValue] (VMType, &\[u8])
    pub fn compose(value: VMValue) -> Option<Self> {
        Some(match value.0 {
            VMType::U8 => Self::U8(value.1[0]),
            VMType::S8 => Self::S8(i8::from_be_bytes((&value.1[..]).try_into().unwrap())),
            VMType::U16 => Self::U16(u16::from_be_bytes((&value.1[..]).try_into().unwrap())),
            VMType::S16 => Self::S16(i16::from_be_bytes((&value.1[..]).try_into().unwrap())),
            VMType::PTR(..)|VMType::U32 => Self::U32(u32::from_be_bytes((&value.1[..]).try_into().unwrap())),
            VMType::S32 => Self::S32(i32::from_be_bytes((&value.1[..]).try_into().unwrap())),
            VMType::U64 => Self::U64(u64::from_be_bytes((&value.1[..]).try_into().unwrap())),
            VMType::S64 => Self::S64(i64::from_be_bytes((&value.1[..]).try_into().unwrap())),
            VMType::U128 => Self::U128(u128::from_be_bytes((&value.1[..]).try_into().unwrap())),
            VMType::S128 => Self::S128(i128::from_be_bytes((&value.1[..]).try_into().unwrap())),
            VMType::F32 => Self::F32(f32::from_be_bytes((&value.1[..]).try_into().unwrap())),
            VMType::F64 => Self::F64(f64::from_be_bytes((&value.1[..]).try_into().unwrap())),
            _ => {return None;}
        })
    }
    /// attempts to decompose the value into a [VMValue] of the target type
    /// 
    /// fails if the target type has a different size than the current type
    pub fn decompose_to<'a>(self, t: VMType<'a>) -> Option<VMValue<'a>> {
        if self.sizeof() != t.sizeof()? {
            return None;
        }
        let bytes: Box<[u8]> = match self {
            CValue::U8(v) => Box::from(v.to_be_bytes()),
            CValue::S8(v) => Box::from(v.to_be_bytes()),
            CValue::U16(v) => Box::from(v.to_be_bytes()),
            CValue::S16(v) => Box::from(v.to_be_bytes()),
            CValue::U32(v) => Box::from(v.to_be_bytes()),
            CValue::S32(v) => Box::from(v.to_be_bytes()),
            CValue::U64(v) => Box::from(v.to_be_bytes()),
            CValue::S64(v) => Box::from(v.to_be_bytes()),
            CValue::U128(v) => Box::from(v.to_be_bytes()),
            CValue::S128(v) => Box::from(v.to_be_bytes()),
            CValue::F32(v) => Box::from(v.to_be_bytes()),
            CValue::F64(v) => Box::from(v.to_be_bytes()),
        };
        return Some(VMValue(t, bytes));
    }
    /// decompose the concrete value into a [VMValue] containing (VMType, &\[u8])
    pub fn decompose<'a>(self) -> VMValue<'a> {
        match self {
            CValue::U8(v) => VMValue(VMType::U8, Box::from(v.to_be_bytes())),
            CValue::S8(v) => VMValue(VMType::S8, Box::from(v.to_be_bytes())),
            CValue::U16(v) => VMValue(VMType::U16, Box::from(v.to_be_bytes())),
            CValue::S16(v) => VMValue(VMType::S16, Box::from(v.to_be_bytes())),
            CValue::U32(v) => VMValue(VMType::U32, Box::from(v.to_be_bytes())),
            CValue::S32(v) => VMValue(VMType::S32, Box::from(v.to_be_bytes())),
            CValue::U64(v) => VMValue(VMType::U64, Box::from(v.to_be_bytes())),
            CValue::S64(v) => VMValue(VMType::S64, Box::from(v.to_be_bytes())),
            CValue::U128(v) => VMValue(VMType::U128, Box::from(v.to_be_bytes())),
            CValue::S128(v) => VMValue(VMType::S128, Box::from(v.to_be_bytes())),
            CValue::F32(v) => VMValue(VMType::F32, Box::from(v.to_be_bytes())),
            CValue::F64(v) => VMValue(VMType::F64, Box::from(v.to_be_bytes())),
        }
    }
    /// converts the underlying value into a byte slice in big endian form
    pub fn to_bytes(&self) -> Box<[u8]> {
        match self {
            CValue::U8(v) => Box::from(v.to_be_bytes()),
            CValue::S8(v) => Box::from(v.to_be_bytes()),
            CValue::U16(v) => Box::from(v.to_be_bytes()),
            CValue::S16(v) => Box::from(v.to_be_bytes()),
            CValue::U32(v) => Box::from(v.to_be_bytes()),
            CValue::S32(v) => Box::from(v.to_be_bytes()),
            CValue::U64(v) => Box::from(v.to_be_bytes()),
            CValue::S64(v) => Box::from(v.to_be_bytes()),
            CValue::U128(v) => Box::from(v.to_be_bytes()),
            CValue::S128(v) => Box::from(v.to_be_bytes()),
            CValue::F32(v) => Box::from(v.to_be_bytes()),
            CValue::F64(v) => Box::from(v.to_be_bytes()),
        }
    }
    /// copies the underlying value into `dest` in big endian representation
    pub fn copy_into(&self, dest: &mut [u8]) -> () {
        match self {
            CValue::U8(v) => {dest.copy_from_slice(&v.to_be_bytes())}
            CValue::S8(v) => {dest.copy_from_slice(&v.to_be_bytes())}
            CValue::U16(v) => {dest.copy_from_slice(&v.to_be_bytes())}
            CValue::S16(v) => {dest.copy_from_slice(&v.to_be_bytes())}
            CValue::U32(v) => {dest.copy_from_slice(&v.to_be_bytes())}
            CValue::S32(v) => {dest.copy_from_slice(&v.to_be_bytes())}
            CValue::U64(v) => {dest.copy_from_slice(&v.to_be_bytes())}
            CValue::S64(v) => {dest.copy_from_slice(&v.to_be_bytes())}
            CValue::U128(v) => {dest.copy_from_slice(&v.to_be_bytes())}
            CValue::S128(v) => {dest.copy_from_slice(&v.to_be_bytes())}
            CValue::F32(v) => {dest.copy_from_slice(&v.to_be_bytes())}
            CValue::F64(v) => {dest.copy_from_slice(&v.to_be_bytes())}
        }
    }
}

impl CValue {
    pub fn add(self, o: Self) -> VMResult<Self> {
        if mem::discriminant(&self) != mem::discriminant(&o) {
            return Err(etodo!());
        }
        Ok(match self {
            Self::U8(v) => Self::U8(v.wrapping_add(match o {Self::U8(n)=>n,_=>unreachable!()})),
            Self::S8(v) => Self::S8(v.wrapping_add(match o {Self::S8(n)=>n,_=>unreachable!()})),
            Self::U16(v) => Self::U16(v.wrapping_add(match o {Self::U16(n)=>n,_=>unreachable!()})),
            Self::S16(v) => Self::S16(v.wrapping_add(match o {Self::S16(n)=>n,_=>unreachable!()})),
            Self::U32(v) => Self::U32(v.wrapping_add(match o {Self::U32(n)=>n,_=>unreachable!()})),
            Self::S32(v) => Self::S32(v.wrapping_add(match o {Self::S32(n)=>n,_=>unreachable!()})),
            Self::U64(v) => Self::U64(v.wrapping_add(match o {Self::U64(n)=>n,_=>unreachable!()})),
            Self::S64(v) => Self::S64(v.wrapping_add(match o {Self::S64(n)=>n,_=>unreachable!()})),
            Self::U128(v) => Self::U128(v.wrapping_add(match o {Self::U128(n)=>n,_=>unreachable!()})),
            Self::S128(v) => Self::S128(v.wrapping_add(match o {Self::S128(n)=>n,_=>unreachable!()})),
            Self::F32(v) => Self::F32(v + match o {Self::F32(n)=>n,_=>unreachable!()}),
            Self::F64(v) => Self::F64(v + match o {Self::F64(n)=>n,_=>unreachable!()}),
        })
    }
    pub fn sub(self, o: Self) -> VMResult<Self> {
        if mem::discriminant(&self) != mem::discriminant(&o) {
            return Err(etodo!());
        }
        Ok(match self {
            Self::U8(v) => Self::U8(v.wrapping_sub(match o {Self::U8(n)=>n,_=>unreachable!()})),
            Self::S8(v) => Self::S8(v.wrapping_sub(match o {Self::S8(n)=>n,_=>unreachable!()})),
            Self::U16(v) => Self::U16(v.wrapping_sub(match o {Self::U16(n)=>n,_=>unreachable!()})),
            Self::S16(v) => Self::S16(v.wrapping_sub(match o {Self::S16(n)=>n,_=>unreachable!()})),
            Self::U32(v) => Self::U32(v.wrapping_sub(match o {Self::U32(n)=>n,_=>unreachable!()})),
            Self::S32(v) => Self::S32(v.wrapping_sub(match o {Self::S32(n)=>n,_=>unreachable!()})),
            Self::U64(v) => Self::U64(v.wrapping_sub(match o {Self::U64(n)=>n,_=>unreachable!()})),
            Self::S64(v) => Self::S64(v.wrapping_sub(match o {Self::S64(n)=>n,_=>unreachable!()})),
            Self::U128(v) => Self::U128(v.wrapping_sub(match o {Self::U128(n)=>n,_=>unreachable!()})),
            Self::S128(v) => Self::S128(v.wrapping_sub(match o {Self::S128(n)=>n,_=>unreachable!()})),
            Self::F32(v) => Self::F32(v - match o {Self::F32(n)=>n,_=>unreachable!()}),
            Self::F64(v) => Self::F64(v - match o {Self::F64(n)=>n,_=>unreachable!()}),
        })
    }
    pub fn mul(self, o: Self) -> VMResult<Self> {
        if mem::discriminant(&self) != mem::discriminant(&o) {
            return Err(etodo!());
        }
        Ok(match self {
            Self::U8(v) => Self::U8(v.wrapping_mul(match o {Self::U8(n)=>n,_=>unreachable!()})),
            Self::S8(v) => Self::S8(v.wrapping_mul(match o {Self::S8(n)=>n,_=>unreachable!()})),
            Self::U16(v) => Self::U16(v.wrapping_mul(match o {Self::U16(n)=>n,_=>unreachable!()})),
            Self::S16(v) => Self::S16(v.wrapping_mul(match o {Self::S16(n)=>n,_=>unreachable!()})),
            Self::U32(v) => Self::U32(v.wrapping_mul(match o {Self::U32(n)=>n,_=>unreachable!()})),
            Self::S32(v) => Self::S32(v.wrapping_mul(match o {Self::S32(n)=>n,_=>unreachable!()})),
            Self::U64(v) => Self::U64(v.wrapping_mul(match o {Self::U64(n)=>n,_=>unreachable!()})),
            Self::S64(v) => Self::S64(v.wrapping_mul(match o {Self::S64(n)=>n,_=>unreachable!()})),
            Self::U128(v) => Self::U128(v.wrapping_mul(match o {Self::U128(n)=>n,_=>unreachable!()})),
            Self::S128(v) => Self::S128(v.wrapping_mul(match o {Self::S128(n)=>n,_=>unreachable!()})),
            Self::F32(v) => Self::F32(v * match o {Self::F32(n)=>n,_=>unreachable!()}),
            Self::F64(v) => Self::F64(v * match o {Self::F64(n)=>n,_=>unreachable!()}),
        })
    }
    pub fn div(self, o: Self) -> VMResult<(Self, Self)> {
        if mem::discriminant(&self) != mem::discriminant(&o) {
            return Err(etodo!());
        }
        let comerr = etodo!();
        Ok(match self {
            Self::U8(v) => {let n = match o {Self::U8(n)=>{if n==0{return Err(comerr);}n},_=>unreachable!()};
                (Self::U8(v.wrapping_div(n)),Self::U8(v.wrapping_rem(n)))}
            Self::S8(v) => {let n = match o {Self::S8(n)=>{if n==0{return Err(comerr);}n},_=>unreachable!()};
                (Self::S8(v.wrapping_div(n)),Self::S8(v.wrapping_rem(n)))}
            Self::U16(v) => {let n = match o {Self::U16(n)=>{if n==0{return Err(comerr);}n},_=>unreachable!()};
                (Self::U16(v.wrapping_div(n)),Self::U16(v.wrapping_rem(n)))}
            Self::S16(v) => {let n = match o {Self::S16(n)=>{if n==0{return Err(comerr);}n},_=>unreachable!()};
                (Self::S16(v.wrapping_div(n)),Self::S16(v.wrapping_rem(n)))}
            Self::U32(v) => {let n = match o {Self::U32(n)=>{if n==0{return Err(comerr);}n},_=>unreachable!()};
                (Self::U32(v.wrapping_div(n)),Self::U32(v.wrapping_rem(n)))}
            Self::S32(v) => {let n = match o {Self::S32(n)=>{if n==0{return Err(comerr);}n},_=>unreachable!()};
                (Self::S32(v.wrapping_div(n)),Self::S32(v.wrapping_rem(n)))}
            Self::U64(v) => {let n = match o {Self::U64(n)=>{if n==0{return Err(comerr);}n},_=>unreachable!()};
                (Self::U64(v.wrapping_div(n)),Self::U64(v.wrapping_rem(n)))}
            Self::S64(v) => {let n = match o {Self::S64(n)=>{if n==0{return Err(comerr);}n},_=>unreachable!()};
                (Self::S64(v.wrapping_div(n)),Self::S64(v.wrapping_rem(n)))}
            Self::U128(v) => {let n = match o {Self::U128(n)=>{if n==0{return Err(comerr);}n},_=>unreachable!()};
                (Self::U128(v.wrapping_div(n)),Self::U128(v.wrapping_rem(n)))}
            Self::S128(v) => {let n = match o {Self::S128(n)=>{if n==0{return Err(comerr);}n},_=>unreachable!()};
                (Self::S128(v.wrapping_div(n)),Self::S128(v.wrapping_rem(n)))}
            Self::F32(v) => {let n = match o {Self::F32(n)=>n,_=>unreachable!()};
                (Self::F32(v / n),Self::F32(v % n))}
            Self::F64(v) => {let n = match o {Self::F64(n)=>n,_=>unreachable!()};
                (Self::F64(v / n),Self::F64(v % n))}
        })
    }
    pub fn shl(self, o: Self) -> VMResult<Self> {
        if mem::discriminant(&self) != mem::discriminant(&o) {
            return Err(etodo!());
        }
        Ok(match self {
            Self::U8(v) => Self::U8(v.wrapping_shl(match o {Self::U8(n)=>n,_=>unreachable!()} as u32)),
            Self::S8(v) => Self::S8(v.wrapping_shl(match o {Self::S8(n)=>n,_=>unreachable!()} as u32)),
            Self::U16(v) => Self::U16(v.wrapping_shl(match o {Self::U16(n)=>n,_=>unreachable!()} as u32)),
            Self::S16(v) => Self::S16(v.wrapping_shl(match o {Self::S16(n)=>n,_=>unreachable!()} as u32)),
            Self::U32(v) => Self::U32(v.wrapping_shl(match o {Self::U32(n)=>n,_=>unreachable!()} as u32)),
            Self::S32(v) => Self::S32(v.wrapping_shl(match o {Self::S32(n)=>n,_=>unreachable!()} as u32)),
            Self::U64(v) => Self::U64(v.wrapping_shl(match o {Self::U64(n)=>n,_=>unreachable!()} as u32)),
            Self::S64(v) => Self::S64(v.wrapping_shl(match o {Self::S64(n)=>n,_=>unreachable!()} as u32)),
            Self::U128(v) => Self::U128(v.wrapping_shl(match o {Self::U128(n)=>n,_=>unreachable!()} as u32)),
            Self::S128(v) => Self::S128(v.wrapping_shl(match o {Self::S128(n)=>n,_=>unreachable!()} as u32)),
            _ => {return Err(etodo!());}
        })
    }
    pub fn shr(self, o: Self) -> VMResult<Self> {
        if mem::discriminant(&self) != mem::discriminant(&o) {
            return Err(etodo!());
        }
        Ok(match self {
            Self::U8(v) => Self::U8(v.wrapping_shr(match o {Self::U8(n)=>n,_=>unreachable!()} as u32)),
            Self::S8(v) => Self::S8((v as u8).wrapping_shr(match o {Self::S8(n)=>n,_=>unreachable!()} as u32) as i8),
            Self::U16(v) => Self::U16(v.wrapping_shr(match o {Self::U16(n)=>n,_=>unreachable!()} as u32)),
            Self::S16(v) => Self::S16((v as u16).wrapping_shr(match o {Self::S16(n)=>n,_=>unreachable!()} as u32) as i16),
            Self::U32(v) => Self::U32(v.wrapping_shr(match o {Self::U32(n)=>n,_=>unreachable!()} as u32)),
            Self::S32(v) => Self::S32((v as u32).wrapping_shr(match o {Self::S32(n)=>n,_=>unreachable!()} as u32) as i32),
            Self::U64(v) => Self::U64(v.wrapping_shr(match o {Self::U64(n)=>n,_=>unreachable!()} as u32)),
            Self::S64(v) => Self::S64((v as u64).wrapping_shr(match o {Self::S64(n)=>n,_=>unreachable!()} as u32) as i64),
            Self::U128(v) => Self::U128(v.wrapping_shr(match o {Self::U128(n)=>n,_=>unreachable!()} as u32)),
            Self::S128(v) => Self::S128((v as u128).wrapping_shr(match o {Self::S128(n)=>n,_=>unreachable!()} as u32) as i128),
            _ => {return Err(etodo!());}
        })
    }
    pub fn sar(self, o: Self) -> VMResult<Self> {
        if mem::discriminant(&self) != mem::discriminant(&o) {
            return Err(etodo!());
        }
        Ok(match self {
            Self::U8(v) => Self::U8((v as i8).wrapping_shr(match o {Self::U8(n)=>n,_=>unreachable!()} as u32) as u8),
            Self::S8(v) => Self::S8(v.wrapping_shr(match o {Self::S8(n)=>n,_=>unreachable!()} as u32)),
            Self::U16(v) => Self::U16((v as i16).wrapping_shr(match o {Self::U16(n)=>n,_=>unreachable!()} as u32) as u16),
            Self::S16(v) => Self::S16(v.wrapping_shr(match o {Self::S16(n)=>n,_=>unreachable!()} as u32)),
            Self::U32(v) => Self::U32((v as i32).wrapping_shr(match o {Self::U32(n)=>n,_=>unreachable!()} as u32) as u32),
            Self::S32(v) => Self::S32(v.wrapping_shr(match o {Self::S32(n)=>n,_=>unreachable!()} as u32)),
            Self::U64(v) => Self::U64((v as i64).wrapping_shr(match o {Self::U64(n)=>n,_=>unreachable!()} as u32) as u64),
            Self::S64(v) => Self::S64(v.wrapping_shr(match o {Self::S64(n)=>n,_=>unreachable!()} as u32)),
            Self::U128(v) => Self::U128((v as i128).wrapping_shr(match o {Self::U128(n)=>n,_=>unreachable!()} as u32) as u128),
            Self::S128(v) => Self::S128(v.wrapping_shr(match o {Self::S128(n)=>n,_=>unreachable!()} as u32)),
            _ => {return Err(etodo!());}
        })
    }
    pub fn xor(self, o: Self) -> VMResult<Self> {
        if mem::discriminant(&self) != mem::discriminant(&o) {
            return Err(etodo!());
        }
        Ok(match self {
            Self::U8(v) => Self::U8(v ^ match o {Self::U8(n)=>n,_=>unreachable!()}),
            Self::S8(v) => Self::S8(v ^ match o {Self::S8(n)=>n,_=>unreachable!()}),
            Self::U16(v) => Self::U16(v ^ match o {Self::U16(n)=>n,_=>unreachable!()}),
            Self::S16(v) => Self::S16(v ^ match o {Self::S16(n)=>n,_=>unreachable!()}),
            Self::U32(v) => Self::U32(v ^ match o {Self::U32(n)=>n,_=>unreachable!()}),
            Self::S32(v) => Self::S32(v ^ match o {Self::S32(n)=>n,_=>unreachable!()}),
            Self::U64(v) => Self::U64(v ^ match o {Self::U64(n)=>n,_=>unreachable!()}),
            Self::S64(v) => Self::S64(v ^ match o {Self::S64(n)=>n,_=>unreachable!()}),
            Self::U128(v) => Self::U128(v ^ match o {Self::U128(n)=>n,_=>unreachable!()}),
            Self::S128(v) => Self::S128(v ^ match o {Self::S128(n)=>n,_=>unreachable!()}),
            _ => {return Err(etodo!());}
        })
    }
    pub fn or(self, o: Self) -> VMResult<Self> {
        if mem::discriminant(&self) != mem::discriminant(&o) {
            return Err(etodo!());
        }
        Ok(match self {
            Self::U8(v) => Self::U8(v | match o {Self::U8(n)=>n,_=>unreachable!()}),
            Self::S8(v) => Self::S8(v | match o {Self::S8(n)=>n,_=>unreachable!()}),
            Self::U16(v) => Self::U16(v | match o {Self::U16(n)=>n,_=>unreachable!()}),
            Self::S16(v) => Self::S16(v | match o {Self::S16(n)=>n,_=>unreachable!()}),
            Self::U32(v) => Self::U32(v | match o {Self::U32(n)=>n,_=>unreachable!()}),
            Self::S32(v) => Self::S32(v | match o {Self::S32(n)=>n,_=>unreachable!()}),
            Self::U64(v) => Self::U64(v | match o {Self::U64(n)=>n,_=>unreachable!()}),
            Self::S64(v) => Self::S64(v | match o {Self::S64(n)=>n,_=>unreachable!()}),
            Self::U128(v) => Self::U128(v | match o {Self::U128(n)=>n,_=>unreachable!()}),
            Self::S128(v) => Self::S128(v | match o {Self::S128(n)=>n,_=>unreachable!()}),
            _ => {return Err(etodo!());}
        })
    }
    pub fn and(self, o: Self) -> VMResult<Self> {
        if mem::discriminant(&self) != mem::discriminant(&o) {
            return Err(etodo!());
        }
        Ok(match self {
            Self::U8(v) => Self::U8(v & match o {Self::U8(n)=>n,_=>unreachable!()}),
            Self::S8(v) => Self::S8(v & match o {Self::S8(n)=>n,_=>unreachable!()}),
            Self::U16(v) => Self::U16(v & match o {Self::U16(n)=>n,_=>unreachable!()}),
            Self::S16(v) => Self::S16(v & match o {Self::S16(n)=>n,_=>unreachable!()}),
            Self::U32(v) => Self::U32(v & match o {Self::U32(n)=>n,_=>unreachable!()}),
            Self::S32(v) => Self::S32(v & match o {Self::S32(n)=>n,_=>unreachable!()}),
            Self::U64(v) => Self::U64(v & match o {Self::U64(n)=>n,_=>unreachable!()}),
            Self::S64(v) => Self::S64(v & match o {Self::S64(n)=>n,_=>unreachable!()}),
            Self::U128(v) => Self::U128(v & match o {Self::U128(n)=>n,_=>unreachable!()}),
            Self::S128(v) => Self::S128(v & match o {Self::S128(n)=>n,_=>unreachable!()}),
            _ => {return Err(etodo!());}
        })
    }
    pub fn cmp(self, o: Self) -> VMResult<(Ordering, Ordering)> {
        if mem::discriminant(&self) != mem::discriminant(&o) {
            return Err(etodo!());
        }
        Ok(match self {
            Self::U8(v) => {let n = match o {Self::U8(n)=>n,_=>unreachable!()};(v.cmp(&n),(v as i8).cmp(&(n as i8)))}
            Self::S8(v) => {let n = match o {Self::S8(n)=>n,_=>unreachable!()};((v as u8).cmp(&(n as u8)),v.cmp(&n))}
            Self::U16(v) => {let n = match o {Self::U16(n)=>n,_=>unreachable!()};(v.cmp(&n),(v as i16).cmp(&(n as i16)))}
            Self::S16(v) => {let n = match o {Self::S16(n)=>n,_=>unreachable!()};((v as u16).cmp(&(n as u16)),v.cmp(&n))}
            Self::U32(v) => {let n = match o {Self::U32(n)=>n,_=>unreachable!()};(v.cmp(&n),(v as i32).cmp(&(n as i32)))}
            Self::S32(v) => {let n = match o {Self::S32(n)=>n,_=>unreachable!()};((v as u32).cmp(&(n as u32)),v.cmp(&n))}
            Self::U64(v) => {let n = match o {Self::U64(n)=>n,_=>unreachable!()};(v.cmp(&n),(v as i64).cmp(&(n as i64)))}
            Self::S64(v) => {let n = match o {Self::S64(n)=>n,_=>unreachable!()};((v as u64).cmp(&(n as u64)),v.cmp(&n))}
            Self::U128(v) => {let n = match o {Self::U128(n)=>n,_=>unreachable!()};(v.cmp(&n),(v as i128).cmp(&(n as i128)))}
            Self::S128(v) => {let n = match o {Self::S128(n)=>n,_=>unreachable!()};((v as u128).cmp(&(n as u128)),v.cmp(&n))}
            Self::F32(v) => {let n = match o {Self::F32(n)=>n,_=>unreachable!()};(v.total_cmp(&n),v.total_cmp(&n))}
            Self::F64(v) => {let n = match o {Self::F64(n)=>n,_=>unreachable!()};(v.total_cmp(&n),v.total_cmp(&n))}
        })
    }
}

impl CValue {
    /// reinterprets the CValue as unsigned, if necessary
    pub fn as_unsigned(self) -> VMResult<Self> {
        Ok(match self {
            Self::U8(v) => Self::U8(v),
            Self::S8(v) => Self::U8(v as u8),
            Self::U16(v) => Self::U16(v),
            Self::S16(v) => Self::U16(v as u16),
            Self::U32(v) => Self::U32(v),
            Self::S32(v) => Self::U32(v as u32),
            Self::U64(v) => Self::U64(v),
            Self::S64(v) => Self::U64(v as u64),
            Self::U128(v) => Self::U128(v),
            Self::S128(v) => Self::U128(v as u128),
            _ => {return Err(etodo!());}
        })
    }
    /// reinterprets the CValue as signed, if necessary
    pub fn as_signed(self) -> VMResult<Self> {
        Ok(match self {
            Self::U8(v) => Self::S8(v as i8),
            Self::S8(v) => Self::S8(v),
            Self::U16(v) => Self::S16(v as i16),
            Self::S16(v) => Self::S16(v),
            Self::U32(v) => Self::S32(v as i32),
            Self::S32(v) => Self::S32(v),
            Self::U64(v) => Self::S64(v as i64),
            Self::S64(v) => Self::S64(v),
            Self::U128(v) => Self::S128(v as i128),
            Self::S128(v) => Self::S128(v),
            _ => {return Err(etodo!());}
        })
    }
    /// attempts to widen the concrete value, panics if the target width is smaller than the current width
    pub fn promote_width(self, t: &Self) -> Self {
        match self {
            Self::U8(v) => match t.sizeof() {
                1 => self,
                2 => Self::U16(v as u16),
                4 => Self::U32(v as u32),
                8 => Self::U64(v as u64),
                16 => Self::U128(v as u128),
                _ => unreachable!()
            }
            Self::S8(v) => match t.sizeof() {
                1 => self,
                2 => Self::S16(v as i16),
                4 => Self::S32(v as i32),
                8 => Self::S64(v as i64),
                16 => Self::S128(v as i128),
                _ => unreachable!()
            }
            Self::U16(v) => match t.sizeof() {
                2 => self,
                4 => Self::U32(v as u32),
                8 => Self::U64(v as u64),
                16 => Self::U128(v as u128),
                _ => panic!("bad promotion")
            }
            Self::S16(v) => match t.sizeof() {
                2 => self,
                4 => Self::S32(v as i32),
                8 => Self::S64(v as i64),
                16 => Self::S128(v as i128),
                _ => panic!("bad promotion")
            }
            Self::U32(v) => match t.sizeof() {
                4 => self,
                8 => Self::U64(v as u64),
                16 => Self::U128(v as u128),
                _ => panic!("bad promotion")
            }
            Self::S32(v) => match t.sizeof() {
                4 => self,
                8 => Self::S64(v as i64),
                16 => Self::S128(v as i128),
                _ => panic!("bad promotion")
            }
            Self::U64(v) => match t.sizeof() {
                8 => self,
                16 => Self::U128(v as u128),
                _ => panic!("bad promotion")
            }
            Self::S64(v) => match t.sizeof() {
                8 => self,
                16 => Self::S128(v as i128),
                _ => panic!("bad promotion")
            }
            Self::U128(_) => match t.sizeof() {
                16 => self,
                _ => panic!("bad promotion")
            }
            Self::S128(_) => match t.sizeof() {
                16 => self,
                _ => panic!("bad promotion")
            }
            _ => panic!("illegal promotion")
        }
    }
}

impl CValue {
    pub unsafe fn u64_unchecked(self) -> u64 {
        match self {
            Self::U64(v) => v,
            _ => unreachable_unchecked()
        }
    }
    pub fn u64(self) -> u64 {
        match self {
            Self::U64(v) => v,
            _ => panic!("attempt to decompose non u64 CValue to u64")
        }
    }
    pub unsafe fn u32_unchecked(self) -> u32 {
        match self {
            Self::U32(v) => v,
            _ => unreachable_unchecked()
        }
    }
    pub fn u32(self) -> u32 {
        match self {
            Self::U32(v) => v,
            _ => panic!("attempt to decompose non u32 CValue to u32")
        }
    }
    pub unsafe fn u16_unchecked(self) -> u16 {
        match self {
            Self::U16(v) => v,
            _ => unreachable_unchecked()
        }
    }
    pub fn u16(self) -> u16 {
        match self {
            Self::U16(v) => v,
            _ => panic!("attempt to decompose non u16 CValue to u16")
        }
    }
    pub unsafe fn u8_unchecked(self) -> u8 {
        match self {
            Self::U8(v) => v,
            _ => unreachable_unchecked()
        }
    }
    pub fn u8(self) -> u8 {
        match self {
            Self::U8(v) => v,
            _ => panic!("attempt to decompose non u8 CValue to u8")
        }
    }
    pub unsafe fn i64_unchecked(self) -> i64 {
        match self {
            Self::S64(v) => v,
            _ => unreachable_unchecked()
        }
    }
    pub fn i64(self) -> i64 {
        match self {
            Self::S64(v) => v,
            _ => panic!("attempt to decompose non s64 CValue to i64")
        }
    }
    pub unsafe fn i32_unchecked(self) -> i32 {
        match self {
            Self::S32(v) => v,
            _ => unreachable_unchecked()
        }
    }
    pub fn i32(self) -> i32 {
        match self {
            Self::S32(v) => v,
            _ => panic!("attempt to decompose non s32 CValue to i32")
        }
    }
    pub unsafe fn i16_unchecked(self) -> i16 {
        match self {
            Self::S16(v) => v,
            _ => unreachable_unchecked()
        }
    }
    pub fn i16(self) -> i16 {
        match self {
            Self::S16(v) => v,
            _ => panic!("attempt to decompose non s16 CValue to i16")
        }
    }
    pub unsafe fn i8_unchecked(self) -> i8 {
        match self {
            Self::S8(v) => v,
            _ => unreachable_unchecked()
        }
    }
    pub fn i8(self) -> i8 {
        match self {
            Self::S8(v) => v,
            _ => panic!("attempt to decompose non s8 CValue to i8")
        }
    }
}


#[non_exhaustive]
#[derive(Debug, Clone)]
/// abstract value
/// 
/// this enum is used to represent types that are either
/// do not have a known size or have variable representations
pub enum AValue<'a> {
    PTR(VMType<'a>, CValue),
    SSTR(String),
    LSTR(String),
    ARR(VMType<'a>, VArr),
    #[non_exhaustive]
    TSTRUCT(())
}

#[derive(Debug, Clone)]
pub enum VArr {
    U8(Box<[u8]>),
    S8(Box<[i8]>),
    U16(Box<[u16]>),
    S16(Box<[i16]>),
    PTR(Box<[u32]>),
    U32(Box<[u32]>),
    S32(Box<[i32]>),
    U64(Box<[u64]>),
    S64(Box<[i64]>),
    U128(Box<[u128]>),
    S128(Box<[i128]>),
    F32(Box<[f32]>),
    F64(Box<[f64]>),
}

