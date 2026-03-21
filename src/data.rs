//! definitions of internal VM datastructures

use std::{fs, io, num::NonZeroU8, ops::Index};

use crate::types::*;

pub struct FlagBit;
impl FlagBit {
    pub const CF_Z: u64 = 1;
    pub const CF_L: u64 = 2;
    pub const CF_B: u64 = 4;
}

pub struct Memory {
    t_length: usize,
    segments: Vec<(Box<[u8]>, u8)>,
    seg_lock: Option<NonZeroU8>,
}

impl Memory {
    pub fn dump(&self, s: &str) -> io::Result<()> {
        fs::write(s, &self.segments[0].0)
    }
}

impl Memory {
    pub const P_READ: u8 = 1;
    pub const P_WRITE: u8 = 2;
    pub const P_EXEC: u8 = 4;

    pub const S_CODE: usize = 0;
    pub const S_INVAR: usize = 1;
    pub const S_DATA: usize = 2;
    pub const S_STACK: usize = 3;

    pub unsafe fn new_uninit() -> Self {
        let mut segments = Vec::new();
        segments.reserve_exact(4);
        segments.resize(4, (Box::from([]), 0));
        Self { t_length: 0, segments, seg_lock: None }
    }
    pub unsafe fn set_segment(&mut self, segment: usize, data: Box<[u8]>) -> () {
        self.t_length -= self.segments[segment].0.len();
        self.t_length += data.len();
        self.segments[segment].0 = data;
    }

    pub fn offset_of(&self, segment: usize) -> usize {
        return self.segments.iter().take(segment).map(|x|x.0.len()).sum();
    }
    pub fn get_perms(&self, segment: usize) -> u8 {
        return self.segments[segment].1;
    }
    pub fn get_perms_at(&self, offset: usize) -> VMResult<u8> {
        return Ok(self.segments[self.get_base_segment(offset)?.0].1);
    }

    pub fn set_perm(&mut self, segment: usize, perm: u8) -> () {
        self.segments[segment].1 |= perm;
    }
    pub fn unset_perm(&mut self, segment: usize, perm: u8) -> () {
        self.segments[segment].1 &= !perm;
    }

    /// returns (base segment, base offset)
    fn get_base_segment(&self, offset: usize) -> VMResult<(usize, usize)> {
        let mut base: usize = 0;
        for i in 0..self.segments.len() {
            if (offset-base) >= self.segments[i].0.len() {
                base += self.segments[i].0.len();
                continue;
            }
            return Ok((i, base));
        }
        return Err(VMError::from_owned(VMErrorClass::Invalid, format!("offset {offset} of {} byte memory out of range", self.offset_of(self.segments.len()))));
    }

    pub fn read(&self, offset: usize, count: usize) -> VMResult<&[u8]> {
        let (seg, base) = self.get_base_segment(offset)?;
        if (offset-base+count) > self.segments[seg].0.len() {
            return Err(VMError::new(VMErrorClass::Boundary, "read crosses segment boundary"));
        }
        return Ok(&self.segments[seg].0[offset-base..offset-base+count]);
    }
    pub fn write(&mut self, offset: usize, data: &[u8]) -> VMResult<()> {
        let count = data.len();
        let (seg, base) = self.get_base_segment(offset)?;
        if (offset-base+count) > self.segments[seg].0.len() {
            return Err(VMError::new(VMErrorClass::Boundary, "write crosses segment boundary"));
        }
        (&mut self.segments[seg].0[offset-base..offset-base+count]).copy_from_slice(data);
        Ok(())
    }
    pub fn read_checked(&self, offset: usize, count: usize) -> VMResult<&[u8]> {
        let (seg, base) = self.get_base_segment(offset)?;
        if (offset-base+count) > self.segments[seg].0.len() {
            return Err(VMError::new(VMErrorClass::Boundary, "read crosses segment boundary"));
        }
        if self.segments[seg].1 & Self::P_READ == 0 {
            return Err(VMError::new(VMErrorClass::Perms, "attempt to read from non-readable memory"));
        }
        return Ok(&self.segments[seg].0[offset-base..offset-base+count]);
    }
    pub fn write_checked(&mut self, offset: usize, data: &[u8]) -> VMResult<()> {
        let count = data.len();
        let (seg, base) = self.get_base_segment(offset)?;
        if (offset-base+count) > self.segments[seg].0.len() {
            return Err(VMError::new(VMErrorClass::Boundary, "write crosses segment boundary"));
        }
        if self.segments[seg].1 & Self::P_WRITE == 0 {
            return Err(VMError::new(VMErrorClass::Perms, "attempt to write to non-writable memory"));
        }
        (&mut self.segments[seg].0[offset-base..offset-base+count]).copy_from_slice(data);
        Ok(())
    }

    /// read a byte, taking segment lock into account
    pub fn get(&self, offset: usize) -> VMResult<u8> {
        if offset >= self.t_length {
            return Err(vmerror!("offset larger than total memory size"));
        }
        let (seg, base) = self.get_base_segment(offset).unwrap();
        if let Some(v) = self.seg_lock {
            if (v.get()-1) as usize != seg {
                return Err(vmerror!("offset not in locked segment"));
            }
        }
        return Ok(self.segments[seg].0[offset-base]);
    }
    /// read a slice, taking segment lock into account
    pub fn get_range<'a>(&'a self, offset: usize, count: usize) -> VMResult<&'a [u8]> {
        if offset >= self.t_length {
            // println!("out of tlen");
            return Err(vmerror!("offset larger than total memory size"));
        }
        let (seg, base) = self.get_base_segment(offset).unwrap();
        if let Some(v) = self.seg_lock {
            if (v.get()-1) as usize != seg {
                // println!("not locked segment");
                return Err(vmerror!("offset not in locked segment"));
            }
            if self.segments[seg].0.len() < offset-base+count {
                return Err(vmerror!(format!("crosses boundary {} / {}", self.segments[seg].0.len(), offset-base+count)));
            }
        }
        return Ok(&self.segments[seg].0[offset-base..offset-base+count]);
    }
    /// lock reads to whatever segment offset is in
    pub fn lock_seg(&mut self, offset: usize) -> VMResult<()> {
        unsafe {
            // sound because usize::add will panic before wraparound occurs
            self.seg_lock = Some(NonZeroU8::new_unchecked(self.get_base_segment(offset)?.0 as u8 + 1));
        }
        Ok(())
    }
    /// unlock reads from segment
    pub fn unlock_seg(&mut self) -> () {
        self.seg_lock = None;
    }
}
impl Index<usize> for Memory {
    type Output = u8;

    fn index(&self, index: usize) -> &Self::Output {
        let mut base: usize = 0;
        for i in 0..self.segments.len() {
            if (index-base) >= self.segments[i].0.len() {
                base += self.segments[i].0.len();
                continue;
            }
            return &self.segments[i].0[index-base];
        }
        panic!("index {index} of {} byte memory out of range", self.t_length);
    }
}

pub struct VMFlags {
    /// whether to lock constant registers
    pub const_lock: bool,
    /// whether the VM has exited
    pub exited: bool,
    /// whether the VM is in priviledged mode
    pub priv_mode: bool,
    /// whether to interpret certain BRK instructions as debugging signals
    pub debug_breaks: bool,
    /// whether to ignore all BRK instructions
    pub ignore_breaks: bool,
}

impl VMFlags {
    pub fn new() -> Self {
        Self { const_lock: true, exited: false, priv_mode: false, debug_breaks: false, ignore_breaks: true }
    }
}

pub(crate) enum VMAction {
    NOP,
    ABORT(VMError),
    STOP,
}

pub(crate) struct InstMods {
    pub era: bool,
    pub call: bool,
    pub oprev: bool,
    pub sign: bool,
    pub fpop: bool,
    pub size: u8,
    pub memoffset: [u8;2],
}

impl InstMods {
    pub fn new() -> Self {
        Self { era: false, call: false, oprev: false, sign: false, fpop: false, size: 2, memoffset: [0,0] }
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy)]
pub enum VMCondition {
    EX,
    NE,
    BX,
    BE,
    LX,
    LE,
    AX,
    AE,
    GX,
    GE,
}

impl VMCondition {
    pub fn check(self, flags: u64) -> bool {
        match self {
            Self::EX => flags & FlagBit::CF_Z != 0,
            Self::NE => flags & FlagBit::CF_Z == 0,
            Self::BX => flags & FlagBit::CF_B != 0,
            Self::BE => flags & (FlagBit::CF_B | FlagBit::CF_Z) != 0,
            Self::LX => flags & FlagBit::CF_L != 0,
            Self::LE => flags & (FlagBit::CF_L | FlagBit::CF_Z) != 0,
            Self::AX => flags & (FlagBit::CF_B | FlagBit::CF_Z) == 0,
            Self::AE => flags & FlagBit::CF_B == 0,
            Self::GX => flags & (FlagBit::CF_L | FlagBit::CF_Z) == 0,
            Self::GE => flags & FlagBit::CF_L == 0,
        }
    }
}
