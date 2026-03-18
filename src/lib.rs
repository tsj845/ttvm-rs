pub mod types;
pub mod vm;
pub mod parser;

pub mod prelude {
    use super::*;
    pub use types::{VMPurpose,VMType,TStructDescriptor};
}
