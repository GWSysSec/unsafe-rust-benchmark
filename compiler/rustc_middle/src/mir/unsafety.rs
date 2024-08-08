//! By default, MIR does not carry unsafety information, which is handled on THIR.
//! However, we may need to do analysis and transformation on unsafe code in MIR
//! or in LLVM. This module contains unsafety related information which is meant
//! to be used in MIR.

use rustc_span::Span;
use rustc_middle::thir::Thir;
use rustc_macros::{HashStable, TyDecodable, TyEncodable, TypeFoldable, TypeVisitable};

#[derive(Clone, TyEncodable, TyDecodable, Debug, HashStable, TypeFoldable, TypeVisitable)]
pub struct UnsafeCode {
    /// Whether this is an unsafe function.
    /// TODO: handle this case.
    pub is_unsafe_fn: bool,
    /// A list of Span of the function's unsafe blocks, if there are any.
    pub unsafe_blocks: Option<Vec::<Span>>
}

impl UnsafeCode {
    /// Create an empty struct of UnsafeCode.
    pub fn new_empty() -> UnsafeCode {
        Self {
            is_unsafe_fn: false,
            unsafe_blocks: None
        }
    }

    /// Collect unsafe code information of a function.
    /// Currently we only collect the Span of unsafe blocks.
    pub fn new<'tcx>(thir: Option<&Thir<'tcx>>) -> UnsafeCode {
        let mut unsafe_code = Self::new_empty();
        if let Some(thir) = thir {
            for block in &thir.blocks {
                match block.safety_mode {
                    rustc_middle::thir::BlockSafety::ExplicitUnsafe(_hir_id) => {
                        if unsafe_code.unsafe_blocks.is_none() {
                            unsafe_code.unsafe_blocks = Some(Vec::new());
                        }
                        unsafe_code.unsafe_blocks.as_mut().unwrap().push(block.span);
                    },
                    _ => {}
                }
            }
        }

        unsafe_code
    }

    /// Check whether an MIR statment/terminator (by its Span) is in an unsafe block.
    pub fn in_unsafe(&self, span: Span) -> bool {
        if let Some(blocks) = &self.unsafe_blocks {
            for block in blocks {
                if block.contains(span) {
                    return true;
                }
            }
        }

        return false;
    }
}