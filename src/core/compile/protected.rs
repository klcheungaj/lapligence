//! Protected (encrypted) source envelopes (IEEE 1800-2009 clause 34).
//!
//! The frontend cannot decrypt: it skips every key, digest and data block of
//! a decryption envelope and parses the text around them, and it parses the
//! plaintext of an encryption envelope as ordinary source. The capture reports
//! each envelope as an owned [`ProtectedEnvelope`] record.
//!
//! Simulation and elaboration checks ([`super::compile_checked`]) reject every
//! envelope: a decryption envelope hides design text the model would need,
//! and an encryption envelope carries protection (licenses, viewports) the
//! simulator cannot honor (user decision 2026-10-27). Each envelope yields one
//! error at its opening directive in the unsupported-by-design message shape;
//! the frontend's per-block "cannot be decrypted and will be skipped" warnings
//! are replaced by those errors. Editor analysis keeps the envelopes as opaque
//! regions instead.

use super::{positions, Diag, Severity, Snapshot};
use crate::ffi::slang::{Diagnostic as SlangDiagnostic, ProtectedEnvelope, ProtectedEnvelopeKind};
use std::collections::HashMap;

/// Frontend name of the warning issued for each skipped key, digest or data
/// block of a protected envelope.
pub const PROTECTED_ENVELOPE_WARNING: &str = "ProtectedEnvelope";

/// Whether `diagnostic` is the frontend's skipped-block warning.
pub fn is_protected_envelope_warning(diagnostic: &SlangDiagnostic) -> bool {
    diagnostic.name == PROTECTED_ENVELOPE_WARNING
}

/// The opening directive spelling of `envelope`, for messages.
pub fn envelope_directive(envelope: &ProtectedEnvelope) -> &'static str {
    match (envelope.kind, envelope.legacy) {
        (ProtectedEnvelopeKind::Decryption, false) => "`pragma protect begin_protected",
        (ProtectedEnvelopeKind::Decryption, true) => "`protected",
        (ProtectedEnvelopeKind::Encryption, false) => "`pragma protect begin",
        (ProtectedEnvelopeKind::Encryption, true) => "`protect",
    }
}

/// Family label of the rejection, as `docs/sim_features.md` names it.
pub const PROTECTED_SOURCE_FAMILY: &str = "protected source envelopes";

/// The unsupported-by-design rejection of `envelope`: `unsupported:
/// <construct> (protected source envelopes) is not supported by llg`, the
/// message shape of every construct `llg` rejects by design.
fn unsupported_message(envelope: &ProtectedEnvelope) -> String {
    let what = match envelope.kind {
        ProtectedEnvelopeKind::Decryption => "protected envelope",
        ProtectedEnvelopeKind::Encryption => "encryption envelope",
    };
    let unterminated = if envelope.unterminated {
        "unterminated "
    } else {
        ""
    };
    format!(
        "unsupported: {unterminated}{what} `` {}`` ({PROTECTED_SOURCE_FAMILY}) is not supported \
         by llg",
        envelope_directive(envelope)
    )
}

/// One error per protected envelope of `snapshot`, located at its opening
/// directive, in envelope order.
pub fn protected_source_diagnostics(snapshot: &Snapshot) -> Vec<Diag> {
    let mut offsets: HashMap<u64, Vec<u64>> = HashMap::new();
    for envelope in &snapshot.protected_envelopes {
        offsets
            .entry(envelope.range.file_id)
            .or_default()
            .push(envelope.range.start);
    }
    let mut positions: HashMap<u64, std::vec::IntoIter<(u32, u32)>> = HashMap::new();
    for file in &snapshot.files {
        if let Some(starts) = offsets.get(&file.id) {
            positions.insert(
                file.id,
                positions::one_based_utf16_positions(&file.text, starts).into_iter(),
            );
        }
    }
    snapshot
        .protected_envelopes
        .iter()
        .map(|envelope| {
            let file = snapshot
                .files
                .iter()
                .find(|file| file.id == envelope.range.file_id);
            let (line, col) = positions
                .get_mut(&envelope.range.file_id)
                .and_then(Iterator::next)
                .unwrap_or((0, 0));
            Diag {
                severity: Severity::Error,
                file: file.map(|file| file.name.clone()),
                line,
                col,
                message: unsupported_message(envelope),
                logical: None,
            }
        })
        .collect()
}

/// The checked-compile diagnostics of a snapshot with protected envelopes:
/// one error per envelope, then the compile projection `diagnostics` without
/// the frontend's skipped-block warnings.
pub(super) fn reject_protected_source(
    snapshot: &Snapshot,
    mut diagnostics: Vec<Diag>,
) -> Vec<Diag> {
    super::retain_frontend_diagnostics(snapshot, &mut diagnostics, |_, frontend, _| {
        !is_protected_envelope_warning(frontend)
    });
    let mut result = protected_source_diagnostics(snapshot);
    result.append(&mut diagnostics);
    result
}

#[cfg(test)]
mod tests {
    use super::super::{compile_sources, compile_sources_checked, CompileError, OwnedSource};
    use super::*;

    const ENVELOPE: &str = "`pragma protect begin_protected\n\
`pragma protect encoding=(enctype=\"base64\", line_length=64, bytes=12)\n\
`pragma protect data_block\n\
QUJDREVGR0hJSktM\n\
`pragma protect end_protected\n";

    fn sources() -> [OwnedSource; 1] {
        [OwnedSource::compilation_unit(
            "prot.sv",
            format!("module m(input logic a);\n{ENVELOPE}endmodule\n\n// tail\n{ENVELOPE}"),
        )]
    }

    /// The owned capture reports both envelopes with their declarations; an
    /// unchecked compile keeps the frontend's skipped-block warnings.
    #[test]
    fn unchecked_compile_keeps_envelopes_and_frontend_warnings() {
        let out = compile_sources(&sources(), &Default::default()).expect("compile");
        let envelopes = &out.snapshot.protected_envelopes;
        assert_eq!(envelopes.len(), 2, "{envelopes:?}");
        assert!(envelopes[0].scope.is_some() && envelopes[1].scope.is_none());
        assert!(envelopes.iter().all(|envelope| envelope.kind
            == ProtectedEnvelopeKind::Decryption
            && !envelope.legacy
            && !envelope.unterminated));
        assert_eq!(
            out.snapshot
                .diagnostics
                .iter()
                .filter(|diagnostic| is_protected_envelope_warning(diagnostic))
                .count(),
            2
        );
    }

    /// A checked compile rejects each envelope at its opening directive and
    /// drops the frontend's skipped-block warnings.
    #[test]
    fn checked_compile_rejects_each_envelope_once() {
        let Err(CompileError::FrontendDiagnostics(diagnostics)) =
            compile_sources_checked(&sources(), &Default::default())
        else {
            panic!("protected source must be rejected");
        };
        let errors: Vec<_> = diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Error)
            .map(|diagnostic| (diagnostic.line, diagnostic.col, diagnostic.message.as_str()))
            .collect();
        let message = "unsupported: protected envelope `` `pragma protect begin_protected`` \
                       (protected source envelopes) is not supported by llg";
        assert_eq!(errors, vec![(2, 1, message), (10, 1, message)]);
        assert!(diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("cannot be decrypted")));
    }
}
