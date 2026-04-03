use std::env::args;
use std::fs;

use territopple_vm::data::Memory;
use territopple_vm::prelude::*;
use territopple_vm::types::{VMResult, VMValue};
use territopple_vm::vm::TTVMFuncBinder;

fn main() -> VMResult<()> {
    // let filedata = fs::read("rs_test.ttvm").unwrap();
    let filedata = fs::read("bot_test.ttvm").unwrap();
    let mut vm = TTVM::from_object_file(&filedata[..], None)?;
    let _ = vm.ext_mem().dump("mem_dump.bin");
    if args().any(|x|x=="--dump") {
        return Ok(());
    }
    // vm.ext_flags().debug_breaks = true;
    // vm.ext_flags().ignore_breaks = false;
    let tc = 25;
    let mut scores: Vec<i32> = Vec::new();
    scores.resize(tc as usize, 0);
    let scores = std::rc::Rc::new(std::cell::RefCell::new(scores));
    let rscores1 = std::rc::Rc::clone(&scores);
    let rscores2 = std::rc::Rc::clone(&scores);
    println!("{:?}", vm.execute("@init", &vec![CValue::U32(25).decompose()], VMType::VOID));
    let bindings = TTVMFuncBinder::new(&vm)
        .bind("scorelegalmoves", &vec![], VMType::VOID, Box::new(move|x|{
            // let o = x.ext_mem().offset_of(Memory::S_STACK);
            // println!("{:?}", x.ext_mem().read(o+4096-32,32)?);
            let mut scores = rscores1.borrow_mut();
            for i in 0..tc {
                scores[i] = CValue::compose(x.execute("_scoremove", &vec![CValue::U32(i as u32).decompose()], VMType::S32)?).
                unwrap().i32();
                // println!("{:?}", x.ext_mem().read(o+4096-32,32)?);
            }
            // x.ext_flags().dev_debug = true;
            // println!("{:#018x}", x.ext_read_reg(Register::SP, VMType::U64).unwrap().u64());
            Ok(CValue::U32(1))
        }))?
        .bind("pickmove", &vec![], VMType::U32, Box::new(move|x|{
            // let o = x.ext_mem().offset_of(Memory::S_STACK);
            // println!("{:?}", x.ext_mem().read(o+4096-32,32)?);
            let mut mv = i32::MIN;
            let rs = rscores2.borrow();
            let mut sscores = rs.iter().enumerate().filter(|a|{if *a.1 >= mv {mv=*a.1;return true;}false}).collect::<Vec<_>>();
            sscores.sort_by(|a,b|b.1.cmp(a.1));
            // println!("{:#018x}", x.ext_read_reg(Register::SP, VMType::U64).unwrap().u64());
            // let c = *sscores[0].1;
            let sscores = sscores.iter().take_while(|a|*a.1 == mv).collect::<Vec<_>>();
            Ok(CValue::U32(sscores[0].0 as u32))
        }))?
        .finish();
    println!("{:?}", vm.bound_execute("@think", &vec![], VMType::U32, Some(&bindings)));
    // vm.ext_flags().halt_breaks = false;
    // vm.ext_flags().ignore_breaks = false;
    // vm.ext_flags().debug_breaks = true;
    // vm.ext_write_reg(Register::R1, CValue::U32(5))?;
    // vm.ext_write_reg(Register::R2, CValue::U32(5))?;
    // println!("{:?}", vm.execute("@constructor", &Vec::new(), VMType::VOID));
    // println!("{:?}", vm.execute("@getrequiredbits", &vec![CValue::U32(8).decompose()], VMType::U8));
    // let naddr = u32::from_be_bytes((vm.execute("@getneighbors", &vec![CValue::U32(8).decompose()], VMType::U32)?.1.as_ref()).try_into().unwrap()) as usize;
    // println!("{:?}", vm.ext_mem().read_avalue(naddr, VMType::UARR(Box::new(VMType::U32))));
    // let paddr = u32::from_be_bytes((vm.execute("@getpositionof", &vec![CValue::U32(8).decompose(), CValue::U16(2).decompose()], VMType::U32)?.1.as_ref()).try_into().unwrap()) as usize;
    // println!("{:?}", vm.ext_mem().read_avalue(paddr, VMType::UARR(Box::new(VMType::U16))));
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

