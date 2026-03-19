use std::fs;

use ttvm_rs::prelude::*;
use ttvm_rs::types::VMResult;

fn main() -> VMResult<()> {
    let filedata = fs::read("rs_test.ttvm").unwrap();
    let mut vm = TTVM::from_object_file(&filedata[..], None)?;
    vm.ext_write_reg(Register::R1, CValue::U32(5))?;
    vm.ext_write_reg(Register::R2, CValue::U32(5))?;
    println!("{:?}", vm.execute("@constructor", &Vec::new(), VMType::VOID));
    // println!("{:?}", vm.ext_read_reg(Register::R0, VMType::U64)?);
    // println!("{:?}", vm.ext_read_reg(Register::R1, VMType::U64)?);
    // println!("{:?}", vm.ext_read_reg(Register::R2, VMType::U64)?);
    // println!("{:?}", vm.ext_read_reg(Register::R3, VMType::U64)?);
    // println!("{:?}", vm.ext_read_reg(Register::R4, VMType::U64)?);
    // let invar = vm.ext_read_reg(Register::INVAR, VMType::U64)?.u64() as usize;
    // for i in 0..6 {
    //     println!("{:?}", vm.ext_mem().read(invar+i*4, 4)?);
    // }
    Ok(())
}

