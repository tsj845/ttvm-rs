//! handles parsing TTVM object files

use std::{collections::HashMap, fmt::{Debug, Display}, str};

use crate::types::*;

#[derive(Clone)]
pub struct VMIndexEntry<'a> {
    name: String,
    pub offset: usize,
    pub params: Box<[(VMType<'a>,String)]>,
    pub rtype: VMType<'a>,
}
impl VMIndexEntry<'_> {
    fn fmt_args(&self) -> String {
        let mut build = String::new();
        let c = self.params.len();
        for i in 0..self.params.len() {
            build.push_str(&format!("{} {}", self.params[i].0, self.params[i].1));
            if i < c-1 {
                build.push_str(", ");
            }
        }
        return build;
    }
    fn formatted(&self) -> String {
        format!("({} @ {:#x} ({}) {}", self.name, self.offset, self.fmt_args(), self.rtype)
    }
}
impl Debug for VMIndexEntry<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // f.debug_struct("VMIndexEntry").field("name", &self.name).field("offset", &self.offset).field("params", &self.params).field("rtype", &self.rtype).finish()
        f.write_str(&self.formatted())
    }
}
impl Display for VMIndexEntry<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.formatted())
    }
}

#[derive(Clone)]
pub(crate) struct PersistData<'a> {
    pub fstr: String,
    pub some: u16,
    pub none: u16,
    pub purpose: VMPurpose,
    pub name: String,
    pub index: HashMap<String, VMIndexEntry<'a>>,
}

impl Debug for PersistData<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PersistData").field("fstr", &self.fstr).field("some", &self.some).field("none", &self.none).field("purpose", &self.purpose).field("name", &self.name).field("index", &self.index).finish()
    }
}

enum DataVar<'a> {
    S32(i32),
    STR(&'a str),
    F64(f64),
    RES(u16),
}

pub(crate) struct ObjectData<'a> {
    fileref: &'a [u8],
    sections: [(usize,usize);4],
    invars: usize,
    datavars: Vec<DataVar<'a>>,
}

impl ObjectData<'_> {
    pub const SUPPORTED_VERSION: u32 = 1;
}

impl<'a> ObjectData<'a> {
    pub fn from_object_file(file: &'a [u8]) -> VMResult<(Self, PersistData<'a>)> {
        if file[0] != Self::SUPPORTED_VERSION as u8 {
            return Err(VMError::from_owned(VMErrorClass::NotSupp, format!("UNSUPPORTED VERSION ({})", file[0])));
        }
        let mut sections = [(0,0);4];
        let mut invars: usize = 0;
        let mut section = 0;
        let mut i: usize = 1;
        let l = file.len();
        let mut p_purpose: VMPurpose = VMPurpose::UKN;
        let mut p_some: u16 = 0;let mut p_none: u16 = 0;
        let mut p_name: String = "".to_string();let mut p_fstr: String = "".to_string();
        let mut datavars = Vec::new();
        let mut index = HashMap::new();
        while i < l {
            if file[i..i+7].eq("SECTION".as_bytes()) {
                i += 7;
                match unsafe {str::from_utf8_unchecked(&file[i..i+5])} {
                    ".conf" => {section = 1;}
                    ".data" => {section = 2;}
                    ".code" => {section = 3;}
                    ".indx" => {section = 4;}
                    _ => {return Err(VMError::new(VMErrorClass::Invalid, "invalid SECTION name"));}
                }
                i += 5;
                let sl = u32::from_be_bytes(file[i..i+4].try_into().unwrap()) as usize;
                i += 4;
                sections[section-1] = (i, i+sl);
                // println!("section {section} from {} to {}", sections[section-1].0, sections[section-1].1);
                continue;
            }
            match section {
                1 => {
                    p_purpose = VMPurpose::from_byte(file[i]);i += 1;
                    p_name = String::from_utf8_lossy(&file[i+1..(i+1+file[i] as usize)]).to_string();i += 1 + file[i] as usize;
                    invars = file[i] as usize;i += 1;
                    p_some = u16::from_be_bytes(file[i..i+2].try_into().unwrap());i += 2;
                    p_none = u16::from_be_bytes(file[i..i+2].try_into().unwrap());i += 2;
                    continue;
                }
                2 => {
                    let sl = u16::from_be_bytes(file[i..i+2].try_into().unwrap()) as usize;
                    p_fstr = String::from_utf8_lossy(&file[i+2..i+2+sl]).to_string();i += 2 + sl;
                    let dc = u16::from_be_bytes(file[i..i+2].try_into().unwrap()) as usize;
                    datavars.reserve_exact(dc);i += 2;
                    for _ in 0..dc {
                        datavars.push(match file[i] {
                            0 => {i+=4;DataVar::S32(i32::from_be_bytes(file[i-4..i].try_into().unwrap()))}
                            1 => {let sl=file[i]as usize;i += 1 + sl;DataVar::STR(unsafe{str::from_utf8_unchecked(&file[i-sl..i])})}
                            2 => {i+=8;DataVar::F64(f64::from_be_bytes(file[i-8..i].try_into().unwrap()))}
                            3 => {i+=2;DataVar::RES(u16::from_be_bytes(file[i-2..i].try_into().unwrap()))}
                            _ => {return Err(VMError::new(VMErrorClass::Invalid, "invalid datavar type id"));}
                        })
                    }
                    continue;
                }
                3 => {
                    i = sections[section-1].1;
                    section = 0;
                    continue;
                }
                4 => {
                    let ic = u16::from_be_bytes(file[i..i+2].try_into().unwrap()) as usize;i += 2;
                    index.reserve(ic);
                    for _ in 0..ic {
                        let mut pnl = Vec::new();let mut ptl = Vec::new();
                        let ename = String::from_utf8_lossy(&file[i+1..i+1+file[i] as usize]).to_string();i += 1 + file[i] as usize;
                        let offset = u32::from_be_bytes(file[i..i+4].try_into().unwrap());i += 4;
                        let rtype: VMType<'_>;
                        match ename.as_str() {
                            "@constructor" => {
                                rtype = VMType::VOID;
                                let pc = file[i] as usize;i += 1;
                                ptl.reserve_exact(pc);
                                pnl.reserve_exact(pc);
                                for _ in 0..pc {
                                    pnl.push(read_sstr(file, &mut i));
                                }
                                ptl.resize(pc, VMType::U32);
                            }
                            "@getpositionof" => {
                                rtype = VMType::UARR(Box::new(VMType::U16));
                                ptl = vec![VMType::U32, VMType::U16];
                                pnl = vec!["tindex", "mode"].iter().map(|x|x.to_string()).collect::<Vec<_>>();
                            }
                            "@getrequiredbits" => {
                                rtype = VMType::U8;
                                ptl = vec![VMType::U32];
                                pnl = vec!["tindex"].iter().map(|x|x.to_string()).collect::<Vec<_>>();
                            }
                            "@getneighbors" => {
                                rtype = VMType::UARR(Box::new(VMType::U32));
                                ptl = vec![VMType::U32];
                                pnl = vec!["tindex"].iter().map(|x|x.to_string()).collect::<Vec<_>>();
                            }
                            "@think" => {
                                rtype = VMType::U32;
                                ptl = vec![VMType::PTR(Box::new(VMType::OSTRUCT))];
                                pnl = vec!["state"].iter().map(|x|x.to_string()).collect::<Vec<_>>();
                            }
                            _ => {
                                let pc = file[i] as usize;i += 1;
                                ptl.reserve_exact(pc);
                                pnl.reserve_exact(pc);
                                for _ in 0..pc {
                                    pnl.push(read_sstr(file, &mut i));
                                }
                                for _ in 0..pc {
                                    ptl.push(read_type(file, &mut i)?);
                                }
                                rtype = read_type(file, &mut i)?;
                            }
                        }
                        index.insert(ename.clone(), VMIndexEntry { name: ename, offset: offset as usize, params: std::iter::zip(ptl, pnl).collect::<Vec<_>>().into_boxed_slice(), rtype });
                    }
                    continue;
                }
                _ => {println!("byte {i} outside section (value {:#x})", file[i]);}
            }
            i += 1;
        }
        Ok((Self { fileref: file, sections, invars, datavars }, PersistData { fstr: p_fstr, some: p_some, none: p_none, purpose: p_purpose, name: p_name, index }))
    }
    pub fn get_code(&self) -> VMResult<Box<[u8]>> {
        return Ok(Box::from(&self.fileref[self.sections[2].0..self.sections[2].1]));
    }
    pub fn get_data(&self) -> VMResult<Box<[u8]>> {
        let mut build = Vec::new();
        for dv in &self.datavars {
            match dv {
                DataVar::S32(v) => {build.extend_from_slice(&v.to_be_bytes())}
                DataVar::STR(s) => {build.push(s.len() as u8);build.extend_from_slice(s.as_bytes());}
                DataVar::F64(v) => {build.extend_from_slice(&v.to_be_bytes());}
                DataVar::RES(n) => {build.reserve(*n as usize);build.resize(build.len()+*n as usize, 0);}
            }
        }
        return Ok(build.into_boxed_slice());
    }
    pub fn get_invar_count(&self) -> usize {
        return self.invars;
    }
}

fn read_sstr<'a>(file: &'a [u8], i: &mut usize) -> String {
    let ci = *i;
    let sl = file[ci] as usize;
    let s = String::from_utf8_lossy(&file[ci+1..ci+1+sl]).to_string();
    *i += 1 + sl;
    return s;
}

fn read_single_type<'a>(cb: u8) -> VMResult<VMType<'a>> {
    // numeric
    if cb & 0x80 != 0 {
        // float
        if cb & 0x20 != 0 {
            match cb & 0x1f {
                4 => {return Ok(VMType::F32);}
                8 => {return Ok(VMType::F64);}
                _ => {return Err(VMError::new(VMErrorClass::Invalid, "float types must be size 4 or 8"));}
            }
        }
        // sint
        if cb & 0x40 != 0 {
            match cb & 0x1f {
                1 => {return Ok(VMType::S8);}
                2 => {return Ok(VMType::S16);}
                4 => {return Ok(VMType::S32);}
                8 => {return Ok(VMType::S64);}
                16 => {return Ok(VMType::S128);}
                _ => {return Err(VMError::new(VMErrorClass::Invalid, "invalid size for sint"));}
            }
        }
        // uint
        match cb & 0x1f {
            1 => {return Ok(VMType::U8);}
            2 => {return Ok(VMType::U16);}
            4 => {return Ok(VMType::U32);}
            8 => {return Ok(VMType::U64);}
            16 => {return Ok(VMType::U128);}
            _ => {return Err(VMError::new(VMErrorClass::Invalid, "invalid size for uint"));}
        }
    }
    // SARR
    if cb & 0x40 != 0 {
        return Ok(VMType::SARR(cb&0x3f, Box::new(VMType::INVALID)));
    }
    return Ok(match cb {
        0 => VMType::VOID,
        1 => VMType::PTR(Box::new(VMType::INVALID)),
        2 => VMType::SSTR,
        3 => VMType::LSTR,
        4 => VMType::UARR(Box::new(VMType::INVALID)),
        5 => VMType::OSTRUCT,
        6 => VMType::TSTRUCT(&TStructDescriptor::UNINIT),
        _ => {return Err(VMError::new(VMErrorClass::Invalid, "invalid type id"));}
    });
}

fn read_type<'a>(file: &'a [u8], i: &mut usize) -> VMResult<VMType<'a>> {
    let mut t = VMType::INVALID;
    let mut ir = *i;
    loop {
        let cb = file[ir];
        let nt = read_single_type(cb)?;
        match nt {
            VMType::PTR(..) => {}
            VMType::SARR(..) => {}
            VMType::UARR(..) => {}
            VMType::TSTRUCT(..) => {unimplemented!()}
            _ => {break;}
        }
        ir += 1;
    }
    let irc = ir;
    let iv = *i;
    *i = irc + 1;
    while ir >= iv {
        let cb = file[ir];
        let nt = read_single_type(cb)?;
        match nt {
            VMType::PTR(..) => {t = VMType::PTR(Box::new(t));}
            VMType::SARR(s,..) => {t = VMType::SARR(s, Box::new(t));}
            VMType::UARR(..) => {t = VMType::UARR(Box::new(t));}
            VMType::TSTRUCT(..) => {unimplemented!()}
            _ => {t = nt;}
        }
        ir -= 1;
    }
    return Ok(t);
}
