# SYN-017 directive qualification

`sim_syn017_directive_effects` runs each positive public `llg` fixture in both
optimizer modes and each listed edition. Expected values follow IEEE 1364-2001
§§2.7, 2.8, 19.2–19.5 and IEEE 1800-2009 §§22.5, 22.11–22.14. The paired
2001 rejections use the older grammar, rather than an output learned from the
frontend.

| Fixture | Edition and effect | Oracle |
| --- | --- | --- |
| `legacy_macro.sv` + `legacy_header.svh` | 2001/2009 parameter substitution, included macro value, `undef`, `ifndef`, generated `[6:0]` range | `value=0000101` |
| `legacy_conditions.sv` | 2001/2009 `ifdef`/`elsif`/`else` with command-line definitions | `choice=11/22/33` |
| `modern_macro.sv` | 2009 paste and stringification in declaration/use; 2001 rejection | `name=value_field value=37` |
| `later_paste.sv`, `later_stringify.sv` | Single-form 2009 positives and 2001 macro-operator rejections | `paste=1`; `quote=value` |
| `later_undefineall.sv`, `later_pragma.sv`, `later_keywords.sv` | 2009 directive positives and 2001 rejections | `undefineall=1`; `pragma=1`; `keywords=1` |
| `inactive_later_forms.sv` | 2001/2009 untaken `ifdef` text containing paste, stringification and `pragma` | `ok`, with no edition diagnostic |
| `else_directive_activity.sv` | 2001 selected `ifdef` ignores later `pragma` in `else`; selected `else` rejects it; 2009 selected `else` executes | `if` / 2001 rejection / `else` |
| `nettype_def.sv`, `nettype_wire.sv`, `nettype_reset.sv` + `nettype_use.sv` | 2001/2009 separate versus merged units; explicit `wire` and `resetall` restore implicit nets | `implicit=z` for separate or restored wire; undeclared-net error for merged `none` |
| `include_order.sv`, `choice.svh`, `later/choice.svh` | 2001/2009 caller directory precedes the later admitted `--include-dir` | `choice=3`, rather than `8` |
| `escaped_range.sv` | 2001/2009 escaped identifier with expanded vector range | `escaped=1000001` |
| `attribute_neutral.sv` | 2001/2009 accepted standard attribute with no dataflow change | `attribute=a` |

The existing `directive_effects` fixtures cover command-line `ifdef`/`elsif`/
`else`, `resetall`, `default_nettype none`, unavailable include rejection,
mapped `line`/`__FILE__`/`__LINE__` output in 2009, physical diagnostic ranges,
`unconnected_drive` pull0/pull1 and `nounconnected_drive`, and advisory
`vectored`/`scalared` values. `sim_syn038_ledger` covers another escaped-name
and simulation-neutral attribute/pragma witness. Library-map `-incdir` remains
SYN-032 Q04, outside this qualification.
