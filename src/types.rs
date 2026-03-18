//! public interface types

use std::fmt::Display;

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
}
impl VMErrorClass {
    pub fn canon_name(&self) -> &'static str {
        match self {
            Self::Other => "Misc",
            Self::NotImpl => "NotImplemented",
            Self::NotSupp => "NotSupported",
            Self::Invalid => "Invalid",
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
impl VMError {
    pub(crate) const DEFAULT: VMError = VMError {class: VMErrorClass::Other, msg: None};
    pub(crate) const UNIMPL: VMError = VMError {class: VMErrorClass::NotImpl, msg: None};
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
#[derive(Debug, Clone)]
pub enum TStructDescriptor<'a> {
    Root(&'a TStructDescriptor<'a>),
    Invalid,
}

impl TStructDescriptor<'static> {
    pub(crate) const INVALID: TStructDescriptor<'static> = TStructDescriptor::Invalid;
    pub(crate) const UNINIT: TStructDescriptor<'static> = TStructDescriptor::Root(&Self::Invalid);
}

#[non_exhaustive]
#[derive(Debug, Clone)]
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
            _ => {return None;}
        })
    }
    pub fn signed(&self) -> bool {
        match self {
            Self::S8|Self::S16|Self::S32|Self::S64|Self::S128=>true,
            _ => false,
        }
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

pub struct VMValue<'a>(pub VMType<'a>, pub &'a [u8]);

pub type VMResult<T> = Result<T, VMError>;

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub(crate) enum Register {
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
}

impl Register {
    pub fn is_cvr(&self) -> bool {
        return (*self >= Self::R13 && *self <= Self::R15) || *self == Self::INVAR || *self == Self::DATA;
    }
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
        return Self::DATA as usize;
    }
}
