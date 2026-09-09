//! Presentation helpers for frontend diagnostics.

use super::compile::Diag;

/// Slang already formats diagnostics for users, so preserve its message.
pub fn user_message(diagnostic: &Diag) -> String {
    diagnostic.message.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::compile::Severity;

    #[test]
    fn preserves_slang_message() {
        let diagnostic = Diag {
            severity: Severity::Error,
            file: Some("source.sv".to_owned()),
            line: 1,
            col: 1,
            message: "expected a statement".to_owned(),
        };
        assert_eq!(user_message(&diagnostic), "expected a statement");
    }
}
