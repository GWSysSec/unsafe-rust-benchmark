//! By default, MIR does not carry unsafety information, which is handled on THIR.
//! However, we may need to do analysis and transformation on unsafe code in MIR
//! or in LLVM. This module contains unsafety related information which is meant
//! to be used in MIR.

use rustc_span::Span;
use rustc_middle::thir::{Thir, BodyTy};
use rustc_hir::Safety;
use rustc_macros::{HashStable, TyDecodable, TyEncodable, TypeFoldable, TypeVisitable};
use super::Body;
use super::ty::TyCtxt;

use phf::phf_set;

#[derive(Clone, TyEncodable, TyDecodable, Debug, HashStable, TypeFoldable, TypeVisitable)]
pub struct UnsafeCode {
    /// Whether this is an unsafe function.
    /// TODO: handle this case.
    pub is_unsafe_fn: bool,
    /// A list of Span of the function's unsafe blocks, if there are any.
    pub unsafe_blocks: Option<Vec::<Span>>
}

/// The set of Rust's native libraries. We ignore analyzing functions in these
/// libraries for three reasons:
///     1. We focus on Rust applications.
///     2. Unsafe code is heavily used (but encapsulated) in native libraries.
///     3. Native libraries are well-vetted and partially formally proved to be safe.
static RUST_NATIVE_LIBS: phf::Set<&'static str> = phf_set! {
    "alloc",
    "backtrace",
    "core",
    "panic_abort",
    "panic_unwind",
    "portable-simd",
    "profiler_builtins",
    "std",
    "stdarch",
    "sysroot",
    "unwind"
};

/// Some native library functions, such as alloc::alloc::exchange_malloc(),
/// are included in and compiled together with the source of the application.
/// We ignore processing a function if it is in Rust's native libraries and
/// if the unsafe_include_native_lib flag is not provided.
pub fn ignore_fn<'tcx>(tcx: TyCtxt<'tcx>, body: &Body<'tcx>) -> bool {
    if tcx.sess.opts.cg.unsafe_include_native_lib {
        return false;
    }
    return RUST_NATIVE_LIBS.contains(tcx.crate_name(body.source.def_id().krate).as_str());
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
    /// 
    /// Currently we only collect two pieces of information:
    /// 1. whether a function is unsafe
    /// 2. the Span of unsafe blocks in a "safe" function.
    pub fn new<'tcx>(thir: Option<&Thir<'tcx>>) -> UnsafeCode {
        let mut unsafe_code = Self::new_empty();

        if let Some(thir) = thir {
            // Check whether this is an unsafe function.
            if let BodyTy::Fn(fn_sig) = thir.body_type {
                if let Safety::Unsafe = fn_sig.safety {
                    unsafe_code.is_unsafe_fn = true;
                    return unsafe_code;
                }
            }

            // Collect unsafe blocks in a "safe" function.
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

    /// Check whether an MIR statment/terminator (by its Span) is in an unsafe fn/block.
    pub fn in_unsafe(&self, span: Span) -> bool {
        if self.is_unsafe_fn {
            return true;
        }

        if let Some(blocks) = &self.unsafe_blocks {
            for block in blocks {
                if block.contains(span) {
                    return true;
                }
            }
        }

        false
    }
}