// UNSAFE-RUST BEGIN
//! MIR pass: StdlibApiTracker
//!
//! Inserts inline asm markers for stdlib API calls inside unsafe blocks.
//! Runs BEFORE MIR inlining so that the markers survive even when the
//! stdlib call itself is inlined away at O2.
//!
//! Each marker is a zero-instruction inline asm whose asm string encodes
//! the API path:
//!
//!     # __unsafe_stdlib_call:std::vec::Vec::<T, A>::set_len
//!
//! The LLVM StdlibApiTracker pass reads these markers and replaces them
//! with calls to the runtime handler `__unsafe_record_stdlib_call()`.

use rustc_ast::InlineAsmOptions;
use rustc_ast::InlineAsmTemplatePiece;
use rustc_index::Idx;
use rustc_middle::mir::*;
use rustc_middle::mir::unsafety;
use rustc_middle::ty::{self, TyCtxt};
use rustc_middle::ty::print::with_no_trimmed_paths;

/// Inline asm marker prefix. The LLVM pass uses this to detect our markers.
const MARKER_PREFIX: &str = "# __unsafe_stdlib_call:";

pub struct StdlibApiTracker;

impl<'tcx> MirPass<'tcx> for StdlibApiTracker {
    fn is_enabled(&self, _sess: &rustc_session::Session) -> bool {
        if std::env::var("UNSAFE_ENABLE_STDLIB_TRACKER").ok().as_deref() != Some("1") {
            return false;
        }
        // UNSAFE_INSTRUMENT_ALL_PACKAGES=1 lifts the primary-package
        // restriction so dependencies are tracked too; see the same override
        // in llvm::isPrimaryPackage(). Absent the variable, only the package
        // cargo marked primary is tracked, as before.
        if std::env::var("UNSAFE_INSTRUMENT_ALL_PACKAGES").ok().as_deref() == Some("1") {
            return true;
        }
        std::env::var("CARGO_PRIMARY_PACKAGE").is_ok()
    }

    fn run_pass(&self, tcx: TyCtxt<'tcx>, body: &mut Body<'tcx>) {
        // Skip native library functions themselves.
        if unsafety::ignore_fn(tcx, body) {
            return;
        }

        let unsafe_code = &body.unsafe_code;

        // Phase 1: Collect stdlib calls in unsafe blocks (immutable scan).
        struct CallSite {
            bb: BasicBlock,
            target: BasicBlock,
            source_info: SourceInfo,
            api_path: String,
        }

        let mut call_sites: Vec<CallSite> = Vec::new();

        for (bb, bb_data) in body.basic_blocks.iter_enumerated() {
            let terminator = bb_data.terminator();

            let (func, target) = match &terminator.kind {
                TerminatorKind::Call { func, target: Some(target), .. } => (func, *target),
                _ => continue,
            };

            let span = terminator.source_info.span;
            if !unsafe_code.in_unsafe(span) {
                continue;
            }

            let func_ty = func.ty(body, tcx);
            if let ty::FnDef(def_id, _) = *func_ty.kind() {
                let crate_name = tcx.crate_name(def_id.krate);
                if unsafety::is_native_lib(crate_name.as_str()) {
                    let path = with_no_trimmed_paths!(tcx.def_path_str(def_id));
                    call_sites.push(CallSite {
                        bb,
                        target,
                        source_info: terminator.source_info,
                        api_path: path,
                    });
                }
            }
        }

        if call_sites.is_empty() {
            return;
        }

        debug!(
            "stdlib_api_tracker: {} stdlib call(s) in {}",
            call_sites.len(),
            tcx.def_path_str(body.source.def_id())
        );

        // Phase 2: Create new basic blocks with inline asm markers.
        let cur_len = body.basic_blocks.len();
        let mut new_blocks: Vec<BasicBlockData<'tcx>> = Vec::new();

        for site in &call_sites {
            let marker_str = format!("{}{}", MARKER_PREFIX, site.api_path);
            let template = tcx.arena.alloc_from_iter([
                InlineAsmTemplatePiece::String(marker_str),
            ]);

            let marker_block = BasicBlockData {
                statements: vec![],
                is_cleanup: false,
                terminator: Some(Terminator {
                    source_info: site.source_info,
                    kind: TerminatorKind::InlineAsm {
                        template,
                        operands: vec![],
                        // No PURE — must have side effects so LLVM never eliminates it.
                        // NOMEM + NOSTACK + PRESERVES_FLAGS = doesn't inhibit
                        // surrounding optimizations.
                        options: InlineAsmOptions::NOMEM
                            | InlineAsmOptions::NOSTACK
                            | InlineAsmOptions::PRESERVES_FLAGS,
                        line_spans: &[],
                        targets: vec![site.target],
                        unwind: UnwindAction::Unreachable,
                    },
                }),
            };

            new_blocks.push(marker_block);
        }

        // Phase 3: Redirect call targets to the new marker blocks.
        let blocks = body.basic_blocks_mut();
        for (i, site) in call_sites.iter().enumerate() {
            let new_idx = BasicBlock::new(cur_len + i);
            let term = blocks[site.bb].terminator_mut();
            if let TerminatorKind::Call { ref mut target, .. } = term.kind {
                *target = Some(new_idx);
            }
        }

        blocks.extend(new_blocks);
    }
}
// UNSAFE-RUST END
