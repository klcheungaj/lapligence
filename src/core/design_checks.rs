//! Design-wide elaboration checks over the owned database that `llg` and
//! `llg_ls` both report: the driver stops on an error, the language server
//! publishes it. Each check states its message once here, worded for the
//! [`Reporter`].

use std::sync::Arc;

use super::db::{Db, TimeScaleElement};
use super::report::{catalog, Message, MessageDef};

/// A source position of a [`DesignDiag`], as the owned database records it:
/// physical file, 1-based line and column, 0 when unknown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DesignLocation {
    pub file: Option<Arc<str>>,
    pub line: u32,
    pub col: u32,
}

/// Another position a [`DesignDiag`] names, with what it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelatedLocation {
    pub location: DesignLocation,
    pub message: String,
}

/// One finding of a design-wide check: a catalog message at a location, with
/// the related positions its text names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DesignDiag {
    pub def: &'static MessageDef,
    pub location: DesignLocation,
    pub text: String,
    pub related: Vec<RelatedLocation>,
}

impl DesignDiag {
    /// The report message the `llg` driver prints: the text at
    /// `file:line:col` (`<unknown>` file, line and column at least 1).
    pub fn message(&self) -> Message {
        let location = &self.location;
        Message::new(self.def, self.text.clone()).location(format!(
            "{}:{}:{}",
            location.file.as_deref().unwrap_or("<unknown>"),
            location.line.max(1),
            location.col.max(1)
        ))
    }
}

/// Which tool reports a [`DesignDiag`]; the text names that tool's way of
/// fixing the finding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reporter {
    /// The `llg` driver: the text is the whole message, positions included.
    Driver,
    /// The `llg_ls` language server: other positions travel as related
    /// locations only (the server maps them from its private staging copies
    /// to the user's files, which it cannot do inside the text).
    Server,
}

/// `ELAB-006`: "It shall be an error if some design elements have a time unit
/// and precision specified and others do not" (IEEE 1800-2009 3.14.2.3). The
/// finding sits at the first element without a time scale and names the first
/// element with one, also as related location; an explicit default time scale
/// gives the others one and makes the design legal (decision F2-D4), so then
/// there is none. `llg` takes the default from `--timescale` (or
/// `compile.timescale` in its `--config`), `llg_ls` from `compile.timescale`
/// in `llg.toml` only.
pub fn mixed_time_scale(db: &Db, reporter: Reporter) -> Option<DesignDiag> {
    let (without, with) = db.mixed_time_scale_elements()?;
    let (at, option) = match reporter {
        Reporter::Driver => (
            format!(
                " at {}:{}:{}",
                with.file.as_deref().unwrap_or("<unknown>"),
                with.line,
                with.col
            ),
            "`--timescale`",
        ),
        Reporter::Server => (String::new(), "`compile.timescale` in llg.toml"),
    };
    Some(DesignDiag {
        def: &catalog::ELAB_MIXED_TIME_SCALE,
        location: element_location(without),
        text: format!(
            "{} `{}` has no `timescale, timeunit or timeprecision, but {} `{}`{at} has a time \
             scale; a design in which only some elements have one needs a default time scale \
             ({option}) (IEEE 1800-2009 3.14.2.3)",
            without.kind, without.name, with.kind, with.name,
        ),
        related: vec![RelatedLocation {
            location: element_location(with),
            message: format!("{} `{}` has a time scale", with.kind, with.name),
        }],
    })
}

fn element_location(element: &TimeScaleElement) -> DesignLocation {
    DesignLocation {
        file: element.file.clone(),
        line: element.line,
        col: element.col,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::compile;

    fn check(text: &str, timescale: Option<&str>, reporter: Reporter) -> Option<DesignDiag> {
        let out = compile::compile_checked(&compile::CompileOpts {
            sources: vec![compile::OwnedSource::compilation_unit("t.sv", text)],
            top: Some("tb".to_owned()),
            default_time_scale: timescale.map(|text| text.parse().expect("time scale")),
            ..Default::default()
        })
        .expect("design should compile");
        let db = Db::from_slang(&out.snapshot).expect("semantic capture");
        mixed_time_scale(&db, reporter)
    }

    const MIXED: &str = "module sub;\nendmodule\n\
                         `timescale 1ns/1ps\n\
                         module tb;\n  sub u();\nendmodule\n";

    #[test]
    fn mixed_design_names_both_elements() {
        let diag = check(MIXED, None, Reporter::Driver).expect("ELAB-006");
        assert_eq!(diag.def.id(), "ELAB-006");
        assert_eq!(diag.location.file.as_deref(), Some("t.sv"));
        assert_eq!((diag.location.line, diag.location.col), (1, 8));
        assert_eq!(
            diag.text,
            "module `sub` has no `timescale, timeunit or timeprecision, but module `tb` at \
             t.sv:4:8 has a time scale; a design in which only some elements have one needs a \
             default time scale (`--timescale`) (IEEE 1800-2009 3.14.2.3)"
        );
        assert_eq!(
            diag.related,
            vec![RelatedLocation {
                location: DesignLocation {
                    file: Some(Arc::from("t.sv")),
                    line: 4,
                    col: 8,
                },
                message: "module `tb` has a time scale".to_owned(),
            }]
        );
        assert_eq!(
            diag.message().render(),
            Message::new(&catalog::ELAB_MIXED_TIME_SCALE, diag.text.clone())
                .location("t.sv:1:8")
                .render()
        );
        let server = check(MIXED, None, Reporter::Server).expect("ELAB-006");
        assert_eq!(
            server.text,
            "module `sub` has no `timescale, timeunit or timeprecision, but module `tb` has a \
             time scale; a design in which only some elements have one needs a default time \
             scale (`compile.timescale` in llg.toml) (IEEE 1800-2009 3.14.2.3)"
        );
        assert_eq!(server.related, diag.related);
    }

    #[test]
    fn no_finding_with_a_default_or_without_mixing() {
        let reporter = Reporter::Driver;
        assert_eq!(check(MIXED, Some("1ns/1ps"), reporter), None);
        assert_eq!(check("module tb;\nendmodule\n", None, reporter), None);
        assert_eq!(
            check(
                "`timescale 1ns/1ps\nmodule tb;\nendmodule\n",
                None,
                reporter
            ),
            None
        );
    }
}
