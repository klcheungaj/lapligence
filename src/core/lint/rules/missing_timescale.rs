//! `missing-timescale` — design elements that take the default time scale.

use crate::core::lint::{LintCtx, LintDiag, LintRule, LintSeverity};

/// Warns once per design when modules, interfaces, programs or packages
/// declare no time scale (no `` `timescale`` directive, `timeunit`,
/// `timeprecision` or compilation-unit time unit applies; IEEE 1800-2009
/// 3.14.2.3) and so take the implementation-specific default, 1ns/1ps
/// (decision F2-D3). The finding is located at the first such element in
/// source order and counts the others. A default chosen with `--timescale`
/// (or `compile.timescale`) is explicit and not reported. `$unit` and nested
/// definitions are not elements of their own: nested ones inherit their
/// parent's scale.
pub struct MissingTimescaleRule;

impl LintRule for MissingTimescaleRule {
    fn id(&self) -> &'static str {
        "missing-timescale"
    }

    fn description(&self) -> &'static str {
        "design elements without a time scale take the default 1ns/1ps"
    }

    fn check(&self, ctx: &LintCtx<'_>) -> Vec<LintDiag> {
        let db = ctx.db;
        if db.default_time_scale_is_explicit() {
            return Vec::new();
        }
        let elements = db.default_time_scale_elements();
        let Some(first) = elements.first() else {
            return Vec::new();
        };
        let default = db.default_time_scale();
        let message = match elements.len() - 1 {
            0 => format!(
                "{} `{}` has no `timescale, timeunit or timeprecision and uses the default \
                 time scale {default}",
                first.kind, first.name
            ),
            others => format!(
                "{} `{}` and {others} other design element{} have no `timescale, timeunit or \
                 timeprecision and use the default time scale {default}",
                first.kind,
                first.name,
                if others == 1 { "" } else { "s" }
            ),
        };
        vec![LintDiag {
            rule: self.id().to_string(),
            logical: None,
            severity: LintSeverity::Warning,
            file: first.file.as_deref().map(str::to_owned),
            line: first.line.max(1),
            col: first.col.max(1),
            message,
        }]
    }
}

#[cfg(test)]
mod tests {
    use crate::core::compile;
    use crate::core::db::Db;
    use crate::core::lint::rules::tests::rule_diags;
    use crate::core::lint::{lint, LintDiag};
    use crate::core::model::DesignModel;

    fn lint_files(files: &[(&str, &str)], timescale: Option<&str>) -> Vec<LintDiag> {
        let out = compile::compile_checked(&compile::CompileOpts {
            sources: files
                .iter()
                .map(|(name, text)| compile::OwnedSource::compilation_unit(*name, *text))
                .collect(),
            top: Some("tb".to_owned()),
            default_time_scale: timescale.map(|text| text.parse().expect("time scale")),
            ..Default::default()
        })
        .expect("design should compile");
        let db = Db::from_slang(&out.snapshot).expect("semantic capture");
        let model = DesignModel::from_db(&db);
        let all = lint(&db, &model);
        rule_diags(&all, "missing-timescale")
            .into_iter()
            .cloned()
            .collect()
    }

    #[test]
    fn one_finding_at_the_first_element_counts_the_others() {
        let diags = lint_files(
            &[
                (
                    "a.sv",
                    "package p;\n  localparam int W = 4;\nendpackage\n\
                     interface ifc;\n  logic a;\nendinterface\n",
                ),
                (
                    "b.sv",
                    "module sub;\nendmodule\n\
                     module tb;\n  ifc i();\n  sub u0();\n  sub u1();\n  initial $display(p::W);\nendmodule\n",
                ),
            ],
            None,
        );
        assert_eq!(diags.len(), 1, "{diags:?}");
        assert_eq!(diags[0].file.as_deref(), Some("a.sv"));
        assert_eq!((diags[0].line, diags[0].col), (1, 9));
        assert_eq!(
            diags[0].message,
            "package `p` and 3 other design elements have no `timescale, timeunit or \
             timeprecision and use the default time scale 1ns/1ps"
        );
    }

    #[test]
    fn declared_elements_are_not_counted() {
        let diags = lint_files(
            &[(
                "t.sv",
                "module sub;\nendmodule\n\
                 `timescale 1ns/1ps\n\
                 module tb;\n  sub u();\nendmodule\n",
            )],
            None,
        );
        assert_eq!(diags.len(), 1, "{diags:?}");
        assert_eq!((diags[0].line, diags[0].col), (1, 8));
        assert_eq!(
            diags[0].message,
            "module `sub` has no `timescale, timeunit or timeprecision and uses the default \
             time scale 1ns/1ps"
        );
    }

    #[test]
    fn quiet_when_every_element_declares_or_the_default_is_explicit() {
        let declared = "`timescale 1ns/1ps\nmodule sub;\nendmodule\n\
                        module tb;\n  timeunit 10ns;\n  timeprecision 1ns;\n  sub u();\nendmodule\n";
        assert!(lint_files(&[("t.sv", declared)], None).is_empty());
        // A compilation-unit time unit covers every element of the unit, and
        // a nested definition inherits from its parent.
        let unit = "timeunit 1ns;\ntimeprecision 1ps;\n\
                    module tb;\n  module inner;\n  endmodule\n  inner i();\nendmodule\n";
        assert!(lint_files(&[("t.sv", unit)], None).is_empty());
        // `$unit` items are not design elements of their own.
        let unit_function = "function automatic int twice(int x);\n  return 2 * x;\nendfunction\n\
                             `timescale 1ns/1ps\nmodule tb;\n  initial $display(twice(2));\nendmodule\n";
        assert!(lint_files(&[("t.sv", unit_function)], None).is_empty());
        let missing = "module tb;\nendmodule\n";
        assert_eq!(lint_files(&[("t.sv", missing)], None).len(), 1);
        assert!(lint_files(&[("t.sv", missing)], Some("1ns/1ps")).is_empty());
        assert!(lint_files(&[("t.sv", missing)], Some("10us/1ns")).is_empty());
    }
}
