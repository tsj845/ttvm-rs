#[macro_use]
pub mod types;
pub mod vm;
pub mod parser;
pub mod data;

pub mod prelude {
    use super::*;
    pub use types::{VMPurpose,VMType,TStructDescriptor,Register,CValue};
    pub use parser::VMIndexEntry;
    pub use vm::TTVM;
}
