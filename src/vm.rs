//! the VM itself

use std::num::NonZeroUsize;

use crate::{parser::{self, PersistData}, types::*};

struct Memory {
    segments: Vec<Box<[u8]>>,
}

struct VMFlags {
    pub const_lock: bool,
    pub exited: bool,
}

/// a VM instance
/// 
/// INVARIANTS:
/// - [Self::memory] MUST contain valid memory segments. In particular, the `code` segment MUST have been produced by the parser or in another way that guarantees the result is valid TTVM bytecode
/// - [Self::regs] MUST be of length `Register::count()*8` initialized according to the TTVM spec
pub struct TTVM<'a> {
    memory: Vec<Box<[u8]>>,
    regs: Box<[u8]>,
    flags: VMFlags,
    persist: PersistData<'a>,
}

impl<'a> TTVM<'a> {
    /// creates a new uninitialized VM instance
    /// this function is unsafe because it leaves the VM in an invalid state
    /// this function does not leave the VM as an invalid bit representation
    /// this function does not leave the VM in a state that would cause UB
    unsafe fn new_uninit(persist: PersistData<'a>) -> Self {
        let mut regs = Vec::new();
        let length = Register::count()*8;
        regs.reserve_exact(length);
        regs.resize(length, 0);
        let mut memory = Vec::with_capacity(4);
        memory.resize(4, Box::from([]));
        Self { memory, regs: regs.into_boxed_slice(), flags: VMFlags::new(), persist }
    }
    pub fn from_object_file(file: &'a [u8], stack_size: Option<NonZeroUsize>) -> VMResult<Self> {
        let (object_data, persist) = parser::ObjectData::from_object_file(file)?;
        let mut raw = unsafe {Self::new_uninit(persist)};
        // load bytecode
        raw.memory[VM_CODE] = object_data.get_code()?;
        // load datavars
        raw.memory[VM_DATA] = object_data.get_data()?;
        // create invariant segment
        let invar_size = object_data.get_invar_count()*4;
        let mut invar = Vec::new();
        invar.reserve_exact(invar_size);
        invar.resize(invar_size, 0);
        raw.memory[VM_INVAR] = invar.into_boxed_slice();
        // create stack
        let mut stack = Vec::new();
        let size = unsafe {stack_size.unwrap_or(NonZeroUsize::new(4096).unwrap_unchecked()).get()};
        stack.reserve_exact(size);
        stack.resize(size, 0);
        raw.memory[VM_STACK] = stack.into_boxed_slice();
        // initialize the constant registers
        raw.flags.const_lock = false;
        raw.write_reg(Register::R13, VMValue(VMType::U64, &([0xff;8][..])))?;
        raw.write_reg(Register::R14, VMValue(VMType::U8, &([0xff][..])))?;
        raw.write_reg(Register::INVAR, VMValue(VMType::PTR(Box::new(VMType::VOID)), &((raw.memory[0].len() as u32).to_be_bytes()[..])))?;
        raw.write_reg(Register::DATA, VMValue(VMType::PTR(Box::new(VMType::VOID)), &(((raw.memory[0].len()+raw.memory[1].len()) as u32).to_be_bytes()[..])))?;
        raw.flags.const_lock = true;
        Ok(raw)
    }
}

impl VMFlags {
    pub fn new() -> Self {
        Self { const_lock: true, exited: false }
    }
}

impl TTVM<'_> {
    fn write_mem(&mut self, addr: usize, value: VMValue) -> VMResult<()> {
        if let Some(size) = value.0.sizeof() {
            let src = value.1;
            (&mut self.regs[addr..addr+size]).clone_from_slice(src);
            return Ok(());
        }
        Err(VMError::DEFAULT)
    }
    fn write_reg(&mut self, register: Register, value: VMValue) -> VMResult<()> {
        let offset = register as usize * 8;
        if register.is_cvr() {
            return Err(VMError::DEFAULT);
        }
        if let Some(size) = value.0.sizeof() {
            (&mut self.regs[offset..offset+8]).fill(0);
            let src = value.1;
            (&mut self.regs[offset+8-size..offset+8]).clone_from_slice(src);
            return Ok(());
        }
        Err(VMError::DEFAULT)
    }
}
