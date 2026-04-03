//! the VM itself

#[cfg(feature = "tokio")]
extern crate tokio;
#[cfg(feature = "tokio")]
use tokio::time::Duration;

use std::{cmp::Ordering, collections::HashMap, fmt::Debug, num::NonZeroUsize};

use crate::{parser::{self, PersistData}, types::*, data::*};

const DT_SIZE_MAP: [VMType<'static>; 4] = [VMType::U8,VMType::U16,VMType::U32,VMType::U64];

macro_rules! opt2err {
    ($inner:expr) => {
        opt2er($inner, VMError::from_owned(VMErrorClass::Other, format!("opt2err failure at {}:{}:{}", file!(), line!(), column!())))
    };
}

pub type ExternVMFunc<'a> = Box<dyn Fn(&mut TTVM) -> VMResult<CValue> + 'a>;
type VMFuncMap<'a> = HashMap<usize, ExternVMFunc<'a>>;

#[derive(Clone)]
pub struct VMExecutionConfig {
    /// a single run of the vm will not exceed this number of cycles
    cycle_limit: usize,
    /// the total number of cycles will not be allowed to exceed this value
    total_cycle_limit: usize,
    #[cfg(feature = "tokio")]
    /// a single run of the vm will not exceed this duration
    timeout: Duration,
    #[cfg(feature = "tokio")]
    /// the sum of all timeouts will not be allowed to exceed this value
    total_timeout: Duration,
}
impl VMExecutionConfig {
    #[cfg(feature = "tokio")]
    pub fn new(cycle_limit: usize, total_cycle_limit: usize, timeout: Duration, total_timeout: Duration) -> Self {
        Self { cycle_limit, total_cycle_limit, timeout, total_timeout }
    }
    #[cfg(not(feature = "tokio"))]
    pub fn new(cycle_limit: usize, total_cycle_limit: usize) -> Self {
        Self { cycle_limit, total_cycle_limit }
    }
}
impl Default for VMExecutionConfig {
    #[cfg(feature = "tokio")]
    fn default() -> Self {
        Self::new(100,100,Duration::from_millis(500),Duration::from_millis(500))
    }
    #[cfg(not(feature = "tokio"))]
    fn default() -> Self {
        Self::new(100,100)
    }
}

#[derive(Clone)]
pub struct CallStack {
    backing: Vec<String>
}
impl Debug for CallStack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.backing.fmt(f)
    }
}
impl CallStack {
    pub fn new() -> Self {
        Self { backing: Vec::new() }
    }
    pub fn push(&mut self, call: String) -> () {
        #[cfg(feature = "debug")]
        println!("Entered call: {call}");
        self.backing.push(call);
    }
    pub fn pop(&mut self) -> Option<String> {
        let ret = self.backing.pop();
        #[cfg(feature = "debug")]
        println!("Exited call: {ret:?}");
        ret
    }
    pub fn clear(&mut self) -> () {
        #[cfg(feature = "debug")]
        println!("Call stack cleared");
        self.backing.clear();
    }
}

#[derive(Clone)]
pub struct TTVM<'a> {
    memory: Memory,
    regs: Box<[u8]>,
    flags: VMFlags,
    persist: PersistData<'a>,
    stack_size: u64,
    config: VMExecutionConfig,
    call_stack: CallStack,
}

pub struct VMExecutionState {
    mem_data: Box<[Box<[u8]>]>,
    regs: Box<[u8]>,
}

pub struct VMBoundFuncs<'a>(VMFuncMap<'a>);

impl VMBoundFuncs<'_> {
    pub fn empty() -> Self {
        Self(HashMap::new())
    }
}

pub struct TTVMFuncBinder<'a, 'b, 'c> {
    vm: &'b TTVM<'a>,
    funcs: HashMap<usize, ExternVMFunc<'c>>,
}
impl<'a, 'b, 'c> TTVMFuncBinder<'a, 'b, 'c> {
    pub fn new(vm: &'b TTVM<'a>) -> Self {
        Self { vm, funcs: HashMap::new() }
    }
    pub fn bind(mut self, symbol: &str, params: &Vec<VMType>, rtype: VMType, func: ExternVMFunc<'c>) -> VMResult<Self> { 
        self.vm.bind_extern(&mut self.funcs, symbol, params, rtype, func)?;
        Ok(self)
    }
    pub fn unbind(mut self, symbol: &str) -> Self {
        self.vm.unbind_extern(&mut self.funcs, symbol);
        self
    }
    pub fn finish(self) -> VMBoundFuncs<'c> {
        // self.vm.bound_funcs = Box::leak(Box::new(self.funcs)) as *const HashMap<usize,ExternVMFunc> as usize;
        VMBoundFuncs(self.funcs)
    }
}

impl<'a> TTVM<'a> {
    fn new_uninit(persist: PersistData<'a>) -> Self {
        let mut regs = Vec::new();
        let length = Register::count()*8;
        regs.reserve_exact(length);
        regs.resize(length, 0);
        let memory = Memory::new_uninit();
        Self { memory, regs: regs.into_boxed_slice(), flags: VMFlags::new(), persist, stack_size: 0, config: VMExecutionConfig::default(), call_stack: CallStack::new() }
    }
    pub fn set_config(&mut self, config: VMExecutionConfig) -> () {
        self.config = config;
    }
    pub fn from_object_file(file: &'a [u8], stack_size: Option<NonZeroUsize>) -> VMResult<Self> {
        let (object_data, persist) = parser::ObjectData::from_object_file(file)?;
        let mut raw = Self::new_uninit(persist);
        // load bytecode
        raw.memory.set_segment(VM_CODE, object_data.get_code()?);
        // load datavars
        raw.memory.set_segment(VM_DATA, object_data.get_data()?);
        // create invariant segment
        let invar_size = object_data.get_invar_count()*4;
        let mut invar = Vec::new();
        invar.reserve_exact(invar_size);
        invar.resize(invar_size, 0);
        raw.memory.set_segment(VM_INVAR, invar.into_boxed_slice());
        // create stack
        let mut stack = Vec::new();
        let size = unsafe {stack_size.unwrap_or(NonZeroUsize::new(4096).unwrap_unchecked()).get()};
        raw.stack_size = size as u64;
        stack.reserve_exact(size);
        stack.resize(size, 0);
        raw.memory.set_segment(VM_STACK, stack.into_boxed_slice());
        // initialize the constant registers
        raw.flags.const_lock = false;
        raw.write_reg(Register::R13, CValue::U64(0xffffffffffffffff))?;
        raw.write_reg(Register::R14, CValue::U8(1))?;
        raw.write_reg(Register::INVAR, CValue::U32(raw.memory.offset_of(VM_INVAR) as u32))?;
        raw.write_reg(Register::DATA, CValue::U32(raw.memory.offset_of(VM_DATA) as u32))?;
        raw.flags.const_lock = true;
        raw.write_reg(Register::SP, CValue::U64(size as u64))?;
        raw.memory.set_perm(Memory::S_CODE, Memory::P_EXEC|Memory::P_READ);
        raw.memory.set_perm(Memory::S_INVAR, Memory::P_READ|Memory::P_WRITE);
        raw.memory.set_perm(Memory::S_DATA, Memory::P_READ|Memory::P_WRITE);
        raw.memory.set_perm(Memory::S_STACK, Memory::P_READ|Memory::P_WRITE);
        Ok(raw)
    }
    fn reset_stack_pointer(&mut self) -> VMResult<()> {
        self.write_reg(Register::SP, CValue::U64(self.stack_size+self.memory.offset_of(Memory::S_STACK) as u64))
    }
}

impl<'a> TTVM<'a> {
    pub fn purpose(&self) -> VMPurpose {
        return self.persist.purpose;
    }
    pub fn name(&'a self) -> &'a str {
        return &self.persist.name;
    }
    pub fn fstr(&'a self) -> &'a str {
        return &self.persist.fstr;
    }
    pub fn some_flags(&self) -> u16 {
        return self.persist.some;
    }
    pub fn none_flags(&self) -> u16 {
        return self.persist.none;
    }
    pub fn call_stack<'b>(&'b self) -> &'b CallStack {
        &self.call_stack
    }
}

impl<'a> TTVM<'a> {
    fn write_reg(&mut self, register: Register, value: CValue) -> VMResult<()> {
        let offset = register as usize * 8;
        if self.flags.const_lock && register.is_cvr() {
            return Err(etodo!());
        }
        let size = value.sizeof();
        // println!("{register:?}");
        (&mut self.regs[offset..offset+8]).fill(0);
        value.copy_into(&mut self.regs[offset+8-size..offset+8]);
        return Ok(());
    }
    fn read_reg(&'a self, register: Register, rtype: VMType<'a>) -> VMResult<CValue> {
        let offset = register as usize * 8;
        if let Some(size) = rtype.sizeof() {
            if let Some(cv) = CValue::from_parts(rtype, &self.regs[offset+8-size..offset+8]) {
                return Ok(cv);
            }
        }
        Err(etodo!())
    }

    /// reads an argument register
    fn read_arg_reg(&self, nf: &mut bool, pc: &mut usize, cb: &mut u8, mods: &InstMods) -> VMResult<Register> {
        *cb = self.memory.get(*pc)?;
        if mods.era {
            *nf = false;
            *pc += 1;
            return Ok(Register::from_byte(*cb));
        } else {
            if *nf {
                *pc += 1;
            }
            *nf = !*nf;
            return Ok(Register::from_byte(if *nf {*cb>>4} else {*cb&0x0f}));
        }
    }
    fn read_arg_nybble(&self, nf: &mut bool, pc: &mut usize, cb: &mut u8) -> VMResult<u8> {
        *cb = self.memory.get(*pc)?;
        if *nf {
            *pc += 1;
        }
        *nf = !*nf;
        return Ok(if *nf {*cb>>4} else {*cb&0x0f});
    }
    fn read_inline_value(&self, length: u8, pc: &mut usize, dtype: VMType) -> VMResult<CValue> {
        if dtype.sizeof().is_none() {
            return Err(etodo!());
        }
        let count = 1<<(length as usize);
        let bytes = self.memory.get_range(*pc, count)?;
        *pc += count;
        let mut vbytes = [0u8;8];
        if dtype.signed() && bytes[0] & 0x80 != 0 {
            (&mut vbytes[0..8-count]).fill(0xff);
        }
        (&mut vbytes[8-count..8]).copy_from_slice(bytes);
        // println!("{dtype} = {count} < {length}");
        // at this point, vbytes now contains a 64 bit value equal to whatever the inline value was
        let r = dtype.sizeof().unwrap();
        // println!("{r}");
        let op = CValue::from_parts(dtype, &vbytes[8-r..8]);
        // println!("{:?}", &op);
        opt2err!(op)
    }

    fn push_value(&mut self, value: CValue) -> VMResult<()> {
        let ss = value.sizeof() as u64;
        if ss > 8 {
            return Err(etodo!());
        }
        let nsp = match self.read_reg(Register::SP, VMType::U64)?{CValue::U64(v)=>v,_=>unreachable!()} - ss;
        self.write_reg(Register::SP, CValue::U64(nsp))?;
        self.memory.write_checked(nsp as usize, &value.to_bytes())?;
        Ok(())
    }
    fn pop_value(&mut self, size: usize) -> VMResult<CValue> {
        if size.count_ones() != 1 {
            return Err(etodo!());
        }
        if size > 8 {
            return Err(etodo!());
        }
        let ss = size.trailing_zeros() as usize;
        let readt = &DT_SIZE_MAP[ss];
        let csp = match self.read_reg(Register::SP, VMType::U64)?{CValue::U64(v)=>v,_=>unreachable!()};
        let olck = self.memory.seg_lock;
        self.memory.lock_seg(self.memory.offset_of(Memory::S_STACK))?;
        let rval = opt2err!(CValue::from_parts(readt.clone(), self.memory.get_range_checked(csp as usize, size)?))?;
        self.memory.unlock_seg();
        self.memory.seg_lock = olck;
        self.write_reg(Register::SP, CValue::U64(csp + size as u64))?;
        Ok(rval)
    }
    fn push_subr(&mut self, reg: Register, size: usize) -> VMResult<()> {
        if size.count_ones() != 1 {
            return Err(etodo!());
        }
        if size > 8 {
            return Err(etodo!());
        }
        let readt = &DT_SIZE_MAP[size.trailing_zeros() as usize];
        let nsp = match self.read_reg(Register::SP, VMType::U64)?{CValue::U64(v)=>v,_=>unreachable!()} - size as u64;
        self.write_reg(Register::SP, CValue::U64(nsp))?;
        let rval = self.read_reg(reg, readt.clone())?;
        self.write_reg(reg, CValue::U8(0))?;
        self.memory.write_checked(nsp as usize, &rval.to_bytes())?;
        Ok(())
    }
    fn pop_subr(&mut self, reg: Register, size: usize) -> VMResult<()> {
        if size.count_ones() != 1 {
            return Err(etodo!());
        }
        if size > 8 {
            return Err(etodo!());
        }
        let readt = &DT_SIZE_MAP[size.trailing_zeros() as usize];
        let csp = match self.read_reg(Register::SP, VMType::U64)?{CValue::U64(v)=>v,_=>unreachable!()};
        let rval = opt2err!(CValue::from_parts(readt.clone(), self.memory.read_checked(csp as usize, size)?))?;
        self.write_reg(Register::SP, CValue::U64(csp + size as u64))?;
        self.write_reg(reg, rval)?;
        Ok(())
    }

    /// executes a single instruction
    fn execute_instruction_impl(&mut self, bound_funcs: &VMBoundFuncs) -> VMResult<VMAction> {
        let mut pc = match self.read_reg(Register::PC, VMType::U64)? {CValue::U64(v)=>v,_=>unreachable!()} as usize;
        if self.memory.get_perms_at(pc)? & Memory::P_EXEC == 0 {
            return Err(vmerror!(format!("attempt to execute from non-executable memory (address {pc:#010x})")));
        }
        self.memory.lock_seg(pc)?;
        let mut mods = InstMods::new();
        let mut cb = self.memory.get(pc)?;
        let mut nf = false; // which nybble to use when reading arguments
        let mut ract = VMAction::NOP;
        while cb & 0x40 != 0 {
            if cb == 0x44 {
                mods.era = true;
            } else if cb == 0x45 {
                mods.call = true;
            } else if cb == 0x46 {
                mods.oprev = true;
            } else if cb == 0x50 {
                mods.sign = true;
            } else if cb == 0x51 {
                mods.fpop = true;
            } else if cb & 0xfc == 0x40 {
                mods.size = cb & 3;
            } else if cb & 0xf8 == 0x48 {
                mods.memoffset[0] = cb & 7;
                if mods.memoffset[0] == 1 {
                    pc += 1;
                    cb = self.memory.get(pc)?;
                    if cb & 0xc0 != 0x40 {
                        return Err(etodo!());
                    }
                    mods.memoffset[1] = cb & 0x3f;
                }
            } else {
                // invalid modifier
                return Err(etodo!());
            }
            pc += 1;
            cb = self.memory.get(pc)?;
        }
        let opcode = cb;
        let _dbgass_pc = pc;
        #[cfg(feature = "debug")]
        {println!("{:#03} -> {_dbgass_pc:#010x} : {:#06x}", self.flags.run_depth, self.read_reg(Register::SP, VMType::U64)?.u64());}
        pc += 1;
        let vt = match mods.size {
            0 => VMType::U8,
            1 => VMType::U16,
            2 => VMType::U32,
            3 => VMType::U64,
            _ => unreachable!()
        };
        match opcode {
            // all of these take r/re,r/re arguments
            0|3|6|12|15|16|17|18|21|24|29|32|43|46 => {
                let rx = self.read_arg_reg(&mut nf, &mut pc, &mut cb, &mods)?;
                let ry = self.read_arg_reg(&mut nf, &mut pc, &mut cb, &mods)?;
                // both of these should pass if two registers were just read
                debug_assert!(nf==false);
                debug_assert!(_dbgass_pc+2==pc||(mods.era&&_dbgass_pc+3==pc));
                match opcode {
                    // these all do an operation of the form f(rx,ry)->rx
                    0|3|6|15|16|17|18|21|24 => {
                        if !self.reg_write_allowed(rx) {
                            return Err(etodo!());
                        }
                        let mut xv;
                        let yv;
                        if opcode <= 6 && mods.fpop {
                            if mods.size < 2 {
                                return Err(etodo!());
                            }
                            return Err(vmerror!("todo", "fp arith operations"));
                        } else if opcode == 6 && mods.sign {
                            xv = self.read_reg(rx, opt2err!(vt.to_signed())?)?;
                            yv = self.read_reg(ry, opt2err!(vt.to_signed())?)?;
                        } else {
                            xv = self.read_reg(rx, vt.clone())?;
                            yv = self.read_reg(ry, vt.clone())?;
                        }
                        match opcode {
                            0 => {xv = xv.add(yv)?;}
                            3 => {xv = xv.sub(yv)?;}
                            6 => {xv = xv.mul(yv)?;}
                            15 => {xv = xv.shl(yv)?;}
                            16 => {xv = xv.shr(yv)?;}
                            17 => {xv = xv.sar(yv)?;}
                            18 => {xv = xv.xor(yv)?;}
                            21 => {xv = xv.or(yv)?;}
                            24 => {xv = xv.and(yv)?;}
                            _ => {unreachable!()}
                        }
                        self.write_reg(rx, xv)?;
                    }
                    12 => {
                        let xv;
                        let yv;
                        if mods.fpop {
                            return Err(vmerror!("todo", "fp div"));
                        } else if mods.sign {
                            xv = self.read_reg(rx, opt2err!(vt.to_signed())?)?;
                            yv = self.read_reg(ry, opt2err!(vt.to_signed())?)?;
                        } else {
                            xv = self.read_reg(rx, vt.clone())?;
                            yv = self.read_reg(ry, vt.clone())?;
                        }
                        let (q, r) = xv.div(yv)?;
                        self.push_value(q)?;
                        self.push_value(r)?;
                    }
                    29 => {
                        let xv;
                        let yv;
                        if mods.fpop {
                            return Err(etodo!());
                        } else {
                            xv = self.read_reg(rx, vt.clone())?;
                            yv = self.read_reg(ry, vt.clone())?;
                        }
                        self.cmp_subr(xv, yv)?;
                    }
                    32 => {
                        if !(self.reg_write_allowed(rx) && self.reg_write_allowed(ry)) {
                            return Err(etodo!());
                        }
                        let xv = self.read_reg(rx, vt.clone())?;
                        let yv = self.read_reg(ry, vt.clone())?;
                        self.write_reg(rx, yv)?;
                        self.write_reg(ry, xv)?;
                    }
                    43 => {
                        if !self.reg_write_allowed(rx) {
                            return Err(etodo!());
                        }
                        if mods.oprev {
                            return Err(etodo!());
                        }
                        let yv;
                        if mods.fpop {
                            return Err(vmerror!("todo", "fp moves"));
                        } else {
                            yv = self.read_reg(ry, vt.clone())?;
                        }
                        self.write_reg(rx, yv)?;
                    }
                    46 => {
                        let mut yv = match self.read_reg(ry, VMType::U64)?{CValue::U64(v)=>v,_=>unreachable!()} as usize;
                        match mods.memoffset[0] {
                            0 => {}
                            1 => {yv += self.read_reg(Register::from_byte(mods.memoffset[1]), VMType::U64)?.u64() as usize;}
                            3 => {yv += self.read_reg(Register::DATA, VMType::U64)?.u64() as usize;}
                            4 => {yv += self.read_reg(Register::INVAR, VMType::U64)?.u64() as usize;}
                            _ => {return Err(etodo!());}
                        }
                        if mods.oprev {
                            let xv = self.read_reg(rx, vt.clone())?;
                            self.memory.write_checked(yv, &xv.to_bytes())?;
                        } else {
                            if !self.reg_write_allowed(rx) {
                                return Err(etodo!());
                            }
                            if mods.fpop {
                                return Err(vmerror!("todo", "fp moves"));
                            }
                            let mv = opt2err!(CValue::from_parts(vt.clone(), self.memory.read_checked(yv, vt.sizeof().unwrap())?))?;
                            self.write_reg(rx, mv)?;
                        }
                    }
                    _ => {unreachable!()}
                }
            }
            // all of these take r/re,m operands
            1|4|7|13|19|22|25|30|44 => {
                // println!("NF: {nf} PC: {pc}");
                let rx = self.read_arg_reg(&mut nf, &mut pc, &mut cb, &mods)?;
                // println!("NF: {nf} PC: {pc} RX: {rx:?}");
                nf = true;
                let mut mz = {
                    let ms = self.read_arg_nybble(&mut nf, &mut pc, &mut cb)?;
                    // println!("NF: {nf} PC: {_dbgass_pc:#010x} MS: {ms} CB: {cb:#04x} OP: {opcode}");
                    self.read_inline_value(ms, &mut pc, match mods.memoffset[0] {
                        0 => VMType::U64,
                        _ => VMType::S64
                    })?
                };
                match mods.memoffset[0] {
                    0 => {}
                    1|3|4 => {
                        let base;
                        if mods.memoffset[0] == 1 {
                            base = self.read_reg(Register::from_byte(mods.memoffset[1]), VMType::S64)?;
                        } else if mods.memoffset[0] == 3 {
                            base = self.read_reg(Register::DATA, VMType::S64)?;
                        } else {
                            base = self.read_reg(Register::INVAR, VMType::S64)?;
                        }
                        mz = mz.add(base)?.as_unsigned()?;
                    }
                    _ => {return Err(etodo!());}
                }
                match opcode {
                    // all of the form f(rx,mz)->rx
                    1|4|7|19|22|25 => {
                        if !self.reg_write_allowed(rx) {
                            return Err(etodo!());
                        }
                        let mut xv;
                        let mv;
                        if opcode <= 7 && mods.fpop {
                            if mods.size < 2 {
                                return Err(etodo!());
                            }
                            return Err(vmerror!("todo", "fp arith ops"));
                        } else if opcode == 7 && mods.sign {
                            return Err(vmerror!("todo", "signed mul"));
                        } else {
                            xv = self.read_reg(rx, vt.clone())?;
                            mv = opt2err!(CValue::from_parts(vt.clone(), self.memory.read_checked(mz.u64() as usize, vt.sizeof().unwrap())?))?;
                        }
                        match opcode {
                            1 => {xv = xv.add(mv)?;}
                            4 => {xv = xv.sub(mv)?;}
                            7 => {xv = xv.mul(mv)?;}
                            19 => {xv = xv.xor(mv)?;}
                            22 => {xv = xv.or(mv)?;}
                            25 => {xv = xv.and(mv)?;}
                            _ => {unreachable!()}
                        }
                        self.write_reg(rx, xv)?;
                    }
                    13 => {
                        let xv;
                        let mv;
                        if mods.fpop {
                            return Err(vmerror!("todo", "fp div"));
                        } else if mods.sign {
                            xv = self.read_reg(rx, opt2err!(vt.to_signed())?)?;
                            mv = opt2err!(CValue::from_parts(vt.to_signed().unwrap(), self.memory.read_checked(mz.u64() as usize, vt.sizeof().unwrap())?))?;
                        } else {
                            xv = self.read_reg(rx, vt.clone())?;
                            mv = opt2err!(CValue::from_parts(vt.clone(), self.memory.read_checked(mz.u64() as usize, vt.sizeof().unwrap())?))?;
                        }
                        let (q, r) = xv.div(mv)?;
                        self.push_value(q)?;
                        self.push_value(r)?;
                    }
                    30 => {
                        let xv;
                        let mv;
                        if mods.fpop {
                            return Err(etodo!());
                        } else {
                            xv = self.read_reg(rx, vt.clone())?;
                            mv = opt2err!(CValue::from_parts(vt.clone(), self.memory.read_checked(mz.u64() as usize, vt.sizeof().unwrap())?))?;
                        }
                        self.cmp_subr(xv, mv)?;
                    }
                    44 => {
                        if mods.oprev {
                            let xv = self.read_reg(rx, vt.clone())?;
                            self.memory.write_checked(mz.u64() as usize, &xv.to_bytes())?;
                        } else {
                            if !self.reg_write_allowed(rx) {
                                return Err(etodo!());
                            }
                            if mods.fpop {
                                return Err(vmerror!("todo", "fp moves"));
                            }
                            let mv = opt2err!(CValue::from_parts(vt.clone(), self.memory.read_checked(mz.u64() as usize, vt.sizeof().unwrap())?))?;
                            self.write_reg(rx, mv)?;
                        }
                    }
                    _ => {unreachable!()}
                }
            }
            // all of these take r/re,i operands
            2|5|8|14|20|23|26|31|45 => {
                let rx = self.read_arg_reg(&mut nf, &mut pc, &mut cb, &mods)?;
                nf = true;
                let iv = {
                    let is = self.read_arg_nybble(&mut nf, &mut pc, &mut cb)?;
                    // println!("NF: {nf} PC: {_dbgass_pc:#010x} IS: {is} CB: {cb:#04x} OP: {opcode}");
                    self.read_inline_value(is, &mut pc, vt.clone())?
                };
                // println!("NPC: {pc:#010x}");
                match opcode {
                    // all of the form f(rx,iz)->rx
                    2|5|8|20|23|26 => {
                        if !self.reg_write_allowed(rx) {
                            return Err(etodo!());
                        }
                        let mut xv;
                        if opcode <= 7 && mods.fpop {
                            if mods.size < 2 {
                                return Err(etodo!());
                            }
                            return Err(vmerror!("todo", "fp arith ops"));
                        } else if opcode == 7 && mods.sign {
                            return Err(vmerror!("todo", "signed mul"));
                        } else {
                            xv = self.read_reg(rx, vt.clone())?;
                        }
                        match opcode {
                            2 => {xv = xv.add(iv)?;}
                            5 => {xv = xv.sub(iv)?;}
                            8 => {xv = xv.mul(iv)?;}
                            20 => {xv = xv.xor(iv)?;}
                            23 => {xv = xv.or(iv)?;}
                            26 => {xv = xv.and(iv)?;}
                            _ => {unreachable!()}
                        }
                        self.write_reg(rx, xv)?;
                    }
                    14 => {
                        let xv;
                        if mods.fpop {
                            return Err(vmerror!("todo", "fp div"));
                        } else if mods.sign {
                            xv = self.read_reg(rx, opt2err!(vt.to_signed())?)?;
                        } else {
                            xv = self.read_reg(rx, vt.clone())?;
                        }
                        let (q, r) = xv.div(iv)?;
                        self.push_value(q)?;
                        self.push_value(r)?;
                    }
                    31 => {
                        let xv;
                        if mods.fpop {
                            return Err(etodo!());
                        } else {
                            xv = self.read_reg(rx, vt.clone())?;
                        }
                        self.cmp_subr(xv, iv)?;
                    }
                    45 => {
                        if !self.reg_write_allowed(rx) {
                            return Err(etodo!());
                        }
                        if mods.oprev {
                            return Err(etodo!());
                        }
                        if mods.fpop {
                            return Err(vmerror!("todo", "fp moves"));
                        }
                        self.write_reg(rx, iv)?;
                    }
                    _ => {unreachable!()}
                }
            }
            33 => {
                let rx = Register::from_byte(self.memory.get(pc)?);
                pc += 1;
                let ry = self.read_arg_reg(&mut nf, &mut pc, &mut cb, &mods)?;
                nf = true;
                let sizes = self.read_arg_nybble(&mut nf, &mut pc, &mut cb)?;
                let mut mw = self.read_inline_value(sizes>>2, &mut pc, match mods.memoffset[0] {
                    0 => VMType::U64,
                    _ => VMType::S64
                })?;
                let mut mz = self.read_inline_value(sizes&3, &mut pc, match mods.memoffset[0] {
                    0 => VMType::U64,
                    _ => VMType::S64
                })?;
                match mods.memoffset[0] {
                    0 => {}
                    1|3|4 => {
                        let base;
                        if mods.memoffset[0] == 1 {
                            base = self.read_reg(Register::from_byte(mods.memoffset[1]), VMType::S64)?;
                        } else if mods.memoffset[0] == 3 {
                            base = self.read_reg(Register::DATA, VMType::S64)?;
                        } else {
                            base = self.read_reg(Register::INVAR, VMType::S64)?;
                        }
                        mw = mw.add(base)?.as_unsigned()?;
                        mz = mz.add(base)?.as_unsigned()?;
                    }
                    _ => {return Err(etodo!());}
                }
                let xv = self.read_reg(rx, vt.clone())?;
                let yv = self.read_reg(ry, vt.clone())?;
                let wv = opt2err!(CValue::from_parts(vt.clone(), self.memory.read_checked(mw.u64() as usize, vt.sizeof().unwrap())?))?;
                self.cmp_subr(xv, wv)?;
                if VMCondition::EX.check(self.read_reg(Register::CF, VMType::U64)?.u64()) {
                    self.memory.write_checked(mz.u64() as usize, &yv.to_bytes())?;
                }
            }
            27 => {
                mods.era = true;
                let rx = self.read_arg_reg(&mut nf, &mut pc, &mut cb, &mods)?;
                self.push_subr(rx, 1 << mods.size)?;
            }
            28 => {
                mods.era = true;
                let rx = self.read_arg_reg(&mut nf, &mut pc, &mut cb, &mods)?;
                self.pop_subr(rx, 1 << mods.size)?;
            }
            // syscall
            42 => {
                match self.read_reg(Register::R0, VMType::U64)?.u64() {
                    6 => {
                        ract = VMAction::STOP;
                    }
                    _ => {return Err(etodo!());}
                }
            }
            // ret
            41 => {
                let raddr = self.pop_value(8)?;
                #[cfg(feature = "debug")]
                {println!("returning to address {:#018x}", raddr.u64());}
                if raddr == CValue::U64(0xffffffffffffffff) {
                    self.write_reg(Register::R1, self.read_reg(Register::R0, VMType::U64)?)?;
                    return Ok(VMAction::STOP);
                }
                let _ = self.call_stack.pop();
                self.write_reg(Register::PC, raddr)?;
            }
            // jmp/jcc
            34|35|36|37|38|39|40 => {
                let rx = Register::from_byte(self.read_arg_nybble(&mut nf, &mut pc, &mut cb)?);
                let jf = self.read_arg_nybble(&mut nf, &mut pc, &mut cb)?;
                let dst;
                let mut index = None;
                if mods.call {
                    if jf & 4 != 0 {
                        if jf & 2 != 0 {
                            if jf & 8 == 0 {
                                dst = self.read_inline_value(2, &mut pc, VMType::U64)?;
                            } else {
                                dst = self.read_inline_value(1, &mut pc, VMType::U64)?;
                            }
                        } else {
                            let i = self.read_inline_value(1, &mut pc, VMType::U64)?.u64() as usize;
                            index = Some(i);
                            dst = CValue::U64(opt2err!(self.persist.index.get(i))?.offset as u64);
                        }
                    } else {
                        if jf & 2 != 0 {
                            dst = self.read_reg(rx, VMType::U64)?;
                        } else {
                            dst = CValue::U64(opt2err!(self.persist.index.get(self.read_reg(rx, VMType::U64)?.u64() as usize))?.offset as u64);
                        }
                    }
                } else {
                    if jf & 4 != 0 {
                        let rf;
                        if jf & 2 != 0 {
                            rf = self.read_inline_value(2, &mut pc, VMType::U64)?;
                        } else {
                            // println!("READING VALUE AT: {pc:#010x}");
                            rf = self.read_inline_value(1, &mut pc, VMType::U64)?;
                            // println!("NEW PC: {pc:#010x}");
                        }
                        if jf & 8 != 0 {
                            dst = CValue::S64(pc as i64).add(rf.as_signed()?)?.as_unsigned()?;
                        } else {
                            dst = rf;
                        }
                    } else {
                        if jf & 8 != 0 {
                            dst = CValue::S64(pc as i64).add(self.read_reg(rx, VMType::S64)?)?.as_unsigned()?;
                        } else {
                            dst = self.read_reg(rx, VMType::U64)?;
                        }
                    }
                }
                // let daddr = dst.u64();
                // println!("OPCODE: {opcode:#010b} PC: {_dbgass_pc:#010x} CALL: {} JF: {jf:#06b} DST: {:#010x}", mods.call, daddr);
                // println!("DBYTE: {:#04x}", self.memory[daddr as usize]);
                let s = jf & 1 != 0;
                let proceed = match opcode {
                    34 => true,
                    _ => match opcode {
                        35 => VMCondition::EX,
                        36 => VMCondition::NE,
                        37 => if s {VMCondition::BX} else {VMCondition::LX}
                        38 => if s {VMCondition::BE} else {VMCondition::LE}
                        39 => if s {VMCondition::AX} else {VMCondition::GX}
                        40 => if s {VMCondition::AE} else {VMCondition::GE}
                        _ => unreachable!()
                    }.check(self.read_reg(Register::CF, VMType::U64)?.u64())
                };
                if proceed {
                    if mods.call {
                        if dst == CValue::U64(0xffffffff) {
                            if let Some(i) = index {
                                // let funcs = mem::replace(&mut self.bound_funcs, HashMap::new());
                                // let funcs = Arc::clone(&self.bound_funcs);
                                // println!("{:?}", bound_funcs.iter().map(|x|(x.0,type_name_of_val(x.1))).collect::<Vec<_>>());
                                self.call_stack.push(format!("Bound function '{}'", &self.persist.index[i].name));
                                if let Some(func) = bound_funcs.0.get(&i) {
                                    self.flags.run_depth += 1;
                                    let v = func(self)?;
                                    self.flags.run_depth -= 1;
                                    self.write_reg(Register::R0, v)?;
                                } else {
                                    return Err(VMError::from_owned(VMErrorClass::ETodo, format!("attempt to call unbound extern function {}", &self.persist.index[i].name)));
                                }
                                // self.bound_funcs = funcs;
                                self.write_reg(Register::PC, CValue::U64(pc as u64))?;
                                return Ok(VMAction::NOP);
                            } else {
                                return Err(VMError::new(VMErrorClass::ETodo, "invalid call address"));
                            }
                        }
                        self.call_stack.push(format!("Function at address {:010x}", dst.u64()));
                        self.push_value(CValue::U64(pc as u64))?;
                    }
                    pc = dst.u64() as usize;
                }
            }
            63 => {return Err(VMError::from_owned(VMErrorClass::Halt, format!("HLT at address {:#010x}", pc-1)));}
            11 => {
                cb = self.memory.get(pc)?;
                pc += 1;
                if !self.flags.ignore_breaks {
                    let mut ndbg = true;
                    if self.flags.debug_breaks {
                        ndbg = false;
                        match cb {
                            201 => {
                                println!("00: {:?}", self.ext_read_reg(Register::R0, VMType::U64)?);
                                println!("01: {:?}", self.ext_read_reg(Register::R1, VMType::U64)?);
                                println!("02: {:?}", self.ext_read_reg(Register::R2, VMType::U64)?);
                                println!("03: {:?}", self.ext_read_reg(Register::R3, VMType::U64)?);
                                println!("04: {:?}", self.ext_read_reg(Register::R4, VMType::U64)?);
                                println!("05: {:?}", self.ext_read_reg(Register::R5, VMType::U64)?);
                                println!("06: {:?}", self.ext_read_reg(Register::R6, VMType::U64)?);
                                println!("07: {:?}", self.ext_read_reg(Register::R7, VMType::U64)?);
                                println!("08: {:?}", self.ext_read_reg(Register::R8, VMType::U64)?);
                                println!("09: {:?}", self.ext_read_reg(Register::R9, VMType::U64)?);
                                println!("10: {:?}", self.ext_read_reg(Register::R10, VMType::U64)?);
                                println!("11: {:?}", self.ext_read_reg(Register::R11, VMType::U64)?);
                                println!("12: {:?}", self.ext_read_reg(Register::R12, VMType::U64)?);
                            }
                            202 => {
                                println!("IV: {:?}", self.ext_read_reg(Register::INVAR, VMType::U64)?);
                                println!("STACK: {}", self.memory.offset_of(Memory::S_STACK));
                            }
                            203 => {
                                println!("CALLS: {:?}", &self.call_stack);
                                let sp = self.read_reg(Register::SP, VMType::U64)?.u64() as usize;
                                println!("SP: {:?}", sp);
                                let o = self.memory.offset_of(Memory::S_STACK);
                                if sp < o+self.stack_size as usize {
                                    let olck = self.memory.seg_lock;
                                    self.memory.seg_lock = None;
                                    println!("STACK: {:?}", self.memory.get_range(sp,self.stack_size as usize+o-sp));
                                    self.memory.seg_lock = olck;
                                }
                            }
                            _ => {ndbg = true;}
                        }
                    }
                    if ndbg {
                        println!("BRK {cb}");
                        if self.flags.halt_breaks {
                            return Err(etodo!());
                        }
                    }
                }
            }
            // invalid opcode
            _ => {return Err(VMError::from_owned(VMErrorClass::Invalid, format!("invalid opcode: {opcode:#04x} @ {_dbgass_pc:#010x}")));}
        }
        self.write_reg(Register::PC, CValue::U64(pc as u64))?;
        Ok(ract)
    }
    /// converts underlying errors into [VMAction::ABORT] instances
    fn execute_instruction(&mut self, bound_funcs: &VMBoundFuncs) -> VMAction {
        let res = self.execute_instruction_impl(bound_funcs);
        self.memory.unlock_seg();
        match res {
            Ok(v) => v,
            Err(e) => VMAction::ABORT(e)
        }
    }
    /// sets up the vm to execute the given symbol
    fn setup_execution(&mut self, symbol: &str, params: &Vec<VMValue>) -> VMResult<()> {
        self.call_stack.push(format!("Extern call to '{symbol}'"));
        // println!("EXECUTING {symbol}");
        if self.flags.run_depth == 0 {
            self.reset_stack_pointer()?;
        }
        self.flags.run_depth += 1;
        self.push_value(CValue::U64(0xffffffffffffffff))?;
        for (i, p) in params.iter().take(4).enumerate() {
            if p.0.sizeof().is_none() {
                return Err(vmerror!("todo", "non-concrete params"));
            }
            self.write_reg(Register::from_byte(i as u8 + 1), opt2err!(CValue::from_parts(p.0.clone(), &p.1))?)?;
        }
        for p in params.iter().skip(4) {
            if p.0.sizeof().is_none() {
                return Err(vmerror!("todo", "non-concrete params"));
            }
            self.push_value(opt2err!(CValue::from_parts(p.0.clone(), &p.1))?)?;
        }
        let pcv = opt2err!(self.persist.index.iter().find(|x|x.name==symbol))?.offset as u64;
        // println!("SYM @ {pcv:#010x}");
        self.write_reg(Register::PC, CValue::U64(pcv))?;
        self.flags.exited = false;
        Ok(())
    }
    fn extract_return<'b>(&mut self, rtype: VMType) -> VMResult<VMValue<'b>> {
        if self.flags.run_depth == 0 {
            self.call_stack.clear();
        } else {
            let _ = self.call_stack.pop();
        }
        if self.flags.exited && match rtype {VMType::VOID=>false,_=>true} {
            if rtype.sizeof().is_some() {
                return Ok(self.read_reg(Register::R1, rtype.clone())?.decompose());
            }
            return Err(vmerror!("todo", "non-conrete return types"));
        }
        Ok(VMValue(VMType::VOID, Box::from([])))
    }
    pub fn execute(&mut self, symbol: &str, params: &Vec<VMValue>, rtype: VMType) -> VMResult<VMValue> {
        self.bound_execute(symbol, params, rtype, None)
    }
    /// begins execution at the specified symbol
    pub fn bound_execute(&mut self, symbol: &str, params: &Vec<VMValue>, rtype: VMType, bindings: Option<&VMBoundFuncs>) -> VMResult<VMValue> {
        self.setup_execution(symbol, params)?;
        // let empty_map = HashMap::new();
        // let map = if self.bound_funcs==0{&empty_map}else{unsafe {&*(self.bound_funcs as *const HashMap<usize, ExternVMFunc>)}};
        let empty = VMBoundFuncs::empty();
        let bound_funcs = bindings.unwrap_or(&empty);
        let mut count = 0usize;
        loop {
            if count > self.config.cycle_limit {
                return Err(vmerror!("todo", "count exceeded"));
            }
            count += 1;
            match self.execute_instruction(bound_funcs) {
                VMAction::NOP => {},
                VMAction::ABORT(e) => {self.flags.run_depth=0;return Err(e);},
                VMAction::STOP => {self.flags.run_depth-=1;self.flags.exited = true;break;}
            }
        }
        self.extract_return(rtype)
    }
    /// binds an external function to the specified symbol
    /// 
    /// fails if the specified signature does not match the actual signature of the symbol
    pub fn bind_extern<'b>(&self, bound_funcs: &mut HashMap<usize, ExternVMFunc<'b>>, symbol: &str, params: &Vec<VMType>, rtype: VMType, func: ExternVMFunc<'b>) -> VMResult<()> {
        let mut e = None;
        for i in 0..self.persist.index.len() {
            let ent = &self.persist.index[i];
            if ent.name == symbol {
                if !ent.params.iter().enumerate().all(|x|params[x.0]==x.1.0) {
                    return Err(VMError::new(VMErrorClass::Invalid, "attempt to bind function with parameter type mismatch"));
                }
                if ent.rtype != rtype {
                    return Err(VMError::new(VMErrorClass::Invalid, "attempt to bind function with return type mismatch"));
                }
                e = Some(i);
                break;
            }
        }
        if let Some(index) = e {
            if let Some(_) = bound_funcs.insert(index, func) {
                return Err(VMError::new(VMErrorClass::ETodo, "attempt to bind to already bound function"));
            }
            return Ok(());
        } else {
            return Err(VMError::new(VMErrorClass::Invalid, "attempt to bind non-existent function"));
        }
    }
    /// removes the function bound to the specified symbol, if any
    pub fn unbind_extern(&self, bound_funcs: &mut HashMap<usize, ExternVMFunc>, symbol: &str) -> () {
        for i in 0..self.persist.index.len() {
            if self.persist.index[i].name == symbol {
                let _ = bound_funcs.remove(&i);
            }
        }
    }
}

#[cfg(feature = "tokio")]
impl<'a> TTVM<'a> {
    pub async fn execute_async(&mut self, symbol: &str, params: &Vec<VMValue<'_>>, rtype: VMType<'_>, timeout: Option<Duration>) -> VMResult<VMValue> {
        self.bound_execute_async(symbol, params, rtype, timeout, None).await
    }
    pub async fn bound_execute_async<'b>(&mut self, symbol: &str, params: &Vec<VMValue<'_>>, rtype: VMType<'_>, timeout: Option<Duration>, bindings: Option<&VMBoundFuncs<'b>>) -> VMResult<VMValue> {
    //     let empty_map = HashMap::new();
    //     let bf = self.bound_funcs;
    //     let map = if self.bound_funcs==0{empty_map}else{unsafe {mem::take(&mut *(self.bound_funcs.clone() as *mut HashMap<usize, ExternVMFunc>))}};
    //     let rv = self._execute_async(&map, symbol, params, rtype, timeout).await;
    //     if bf != 0 {
    //         unsafe {let _ = mem::replace(&mut *(self.bound_funcs.clone() as *mut _), map);}
    //     }
    //     rv
    // }
    // async fn _execute_async<'b>(&mut self, map: &HashMap<usize, ExternVMFunc>, symbol: &str, params: &Vec<VMValue<'_>>, rtype: VMType<'_>, timeout: Option<Duration>) -> VMResult<VMValue<'b>> {
        self.setup_execution(symbol, params)?;
        let timer_duration = timeout.unwrap_or(self.config.timeout);
        let mut total_count = 0;
        let mut total_time = Duration::from_millis(0);
        let empty = VMBoundFuncs::empty();
        let bound_funcs = bindings.unwrap_or(&empty);
        'outer: loop {
            let mut count = 0;
            let timer = tokio::time::sleep(timer_duration);
            'inner: loop {
                if count > self.config.cycle_limit || timer.is_elapsed() {
                    use tokio::time::Instant;

                    total_count += count;
                    total_time += timer.deadline() - Instant::now();
                    tokio::task::yield_now().await;
                    break 'inner;
                }
                match self.execute_instruction(bound_funcs) {
                    VMAction::NOP => {},
                    VMAction::ABORT(e) => {self.flags.run_depth=0;return Err(e);},
                    VMAction::STOP => {self.flags.run_depth-=1;self.flags.exited = true;break 'outer;}
                }
                count += 1;
            }
            if total_count > self.config.total_cycle_limit || total_time > self.config.total_timeout {
                return Err(vmerror!("todo", "count/time exceed"));
            }
        }
        self.extract_return(rtype)
    }
}

impl TTVM<'_> {
    fn cmp_subr(&mut self, xv: CValue, yv: CValue) -> VMResult<()> {
        let (uord, sord) = xv.cmp(yv)?;
        let mut cfv = 0;
        cfv |= match uord {
            Ordering::Equal => FlagBit::CF_Z,
            Ordering::Less => FlagBit::CF_B,
            _ => 0,
        };
        cfv |= match sord {
            Ordering::Equal => FlagBit::CF_Z,
            Ordering::Less => FlagBit::CF_L,
            _ => 0,
        };
        self.write_reg(Register::CF, CValue::U64(cfv))?;
        Ok(())
    }
    fn reg_write_allowed(&self, reg: Register) -> bool {
        return self.flags.priv_mode || !reg.is_ror();
    }
}

fn opt2er<T>(o: Option<T>, e: VMError) -> VMResult<T> {
    match o {
        Some(v) => Ok(v),
        None => Err(e)
    }
}



/// all of these methods directly alter the internal state of the TTVM without regard for its invariants
/// do not use these unless you are familiar with the specific TTVM implementation
impl TTVM<'_> {
    pub fn ext_write_reg(&mut self, register: Register, value: CValue) -> VMResult<()> {
        self.write_reg(register, value)
    }
    pub fn ext_read_reg(&self, register: Register, rtype: VMType) -> VMResult<CValue> {
        self.read_reg(register, rtype)
    }
    pub fn ext_mem(&mut self) -> &mut Memory {
        return &mut self.memory;
    }
    pub fn ext_flags(&mut self) -> &mut VMFlags {
        return &mut self.flags;
    }
    pub fn ext_pushr(&mut self, reg: Register, size: usize) -> VMResult<()> {
        self.push_subr(reg, size)
    }
    pub fn ext_popr(&mut self, reg: Register, size: usize) -> VMResult<()> {
        self.pop_subr(reg, size)
    }
    pub fn ext_pushv(&mut self, value: CValue) -> VMResult<()> {
        self.push_value(value)
    }
    pub fn ext_popv(&mut self, size: usize) -> VMResult<CValue> {
        self.pop_value(size)
    }
    pub fn ext_reset_stack_pointer(&mut self) -> VMResult<()> {
        self.reset_stack_pointer()
    }
}
