use std::env::args;
use std::fs;

use territopple_vm::prelude::*;
use territopple_vm::types::VMResult;

fn main() -> VMResult<()> {
    let filedata = fs::read("rs_test.ttvm").unwrap();
    let mut vm = TTVM::from_object_file(&filedata[..], None)?;
    let _ = vm.ext_mem().dump("mem_dump.bin");
    if args().any(|x|x=="--dump") {
        return Ok(());
    }
    // vm.ext_flags().halt_breaks = false;
    // vm.ext_flags().ignore_breaks = false;
    // vm.ext_flags().debug_breaks = true;
    vm.ext_write_reg(Register::R1, CValue::U32(5))?;
    vm.ext_write_reg(Register::R2, CValue::U32(5))?;
    println!("{:?}", vm.execute("@constructor", &Vec::new(), VMType::VOID));
    println!("{:?}", vm.execute("@getrequiredbits", &vec![CValue::U32(8).decompose()], VMType::U8));
    let naddr = u32::from_be_bytes((vm.execute("@getneighbors", &vec![CValue::U32(8).decompose()], VMType::U32)?.1.as_ref()).try_into().unwrap()) as usize;
    println!("{:?}", vm.ext_mem().read_avalue(naddr, VMType::UARR(Box::new(VMType::U32))));
    let paddr = u32::from_be_bytes((vm.execute("@getpositionof", &vec![CValue::U32(8).decompose(), CValue::U16(2).decompose()], VMType::U32)?.1.as_ref()).try_into().unwrap()) as usize;
    println!("{:?}", vm.ext_mem().read_avalue(paddr, VMType::UARR(Box::new(VMType::U16))));
    // let data = vm.ext_read_reg(Register::DATA, VMType::U64)?.u64() as usize;
    // println!("{data}");
    // println!("{:?}", vm.ext_mem().get_range(data, 17)?);
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

