use std::env::{self, join_paths};
use std::{fs, path, process};

use ttvm_rs::prelude::*;
use ttvm_rs::parser::{VMIndexEntry,ObjectData,PersistData};

fn main() -> () {
    // println!("{:?}", env::current_dir().unwrap());
    let filedata = fs::read("t3r_test.ttvm").unwrap();
    let (obj, per) = ObjectData::from_object_file(&filedata[..]).unwrap();
    println!("{:?}", per);
}

