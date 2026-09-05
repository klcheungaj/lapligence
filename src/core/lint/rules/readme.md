# core/lint/rules

Individual lint policies over the owned `Db` and `DesignModel`. Rules report
findings through `LintCtx`; they must not traverse live VPI data or perform I/O.

Put shared graph/data-flow analysis in the lint analysis layer rather than
duplicating it across rules, and register every new rule with focused behavior
and configuration tests.
