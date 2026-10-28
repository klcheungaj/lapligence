# ADV-030 protected source envelopes

User decision 2026-10-27: encrypted (protected) source is unsupported by `llg`;
`llg_ls` parses and indexes it with the protected text as a black box (its
tests live in `src/bin/llg_ls/features/tests/protected.rs`). `llg` has no keys
and does not decrypt, so every envelope stops the run before elaboration
output with one source-located line at its opening directive

```text
error: <file>:<line>:<col>: unsupported: <construct> (protected source envelopes) is not supported by llg
```

and exit status 1. The encoded text is never skipped silently. The expected
lines are written out literally in `tests/sim_feature_completion/adv_030.rs`;
every negative fixture runs in both optimizer modes and must leave its
`--out-dir` empty. The payloads are pseudo-random base64 under valid protect
pragmas: no real key, digest or ciphertext.

Clause basis (`docs/specification/SystemVerilog-1800-2009.txt`):

- 34.2, lines 54173-54175: "Decryption envelopes specify the pragma
  expressions for decrypting encrypted text regions. A decryption envelope
  begins in the source text when a begin_protected pragma expression is
  encountered. The end of the decryption envelope occurs at the point where an
  end_protected pragma expression is encountered." The error sits at that
  opening directive.
- 34.2, lines 54177-54180: "Decryption envelopes may contain other envelopes
  within their enclosed data block. [...] Code that is contained within a
  decryption envelope is said to be protected." Each nested envelope is
  reported.
- 34.2, lines 54168-54169: "An encryption envelope begins in the source text
  when a begin pragma expression is encountered." 34.5.1.2, lines 54434-54435:
  "ENCRYPTION INPUT: The begin pragma expression is used in the input text to
  indicate to an encrypting tool the point at which encryption shall begin."
  `llg` is not an encrypting tool and would not honor the envelope's
  protection settings, so it rejects encryption envelopes too.
- 34.3, lines 54205-54206: "Tools that process SystemVerilog source code shall
  perform envelope decryption for all decryption envelopes contained in the
  source text, where the proper key is supplied by the user." No key is
  supplied; rejecting is the decision, not an LRM rule.
- 34.4, line 54320: "Protected envelopes are lexical regions delimited by
  protect pragma directives." Protect keywords that open no envelope delimit
  nothing (positive control).

| Fixture | Case | Expected |
| --- | --- | --- |
| `envelope_in_body.sv` | envelope inside a module body, after plaintext | `3:1` protected envelope |
| `whole_module.sv` | envelope around a whole module that `tb` instantiates | `12:1` protected envelope (the frontend's unknown-module error follows) |
| `plaintext_header.sv` | plaintext header; body and `endmodule` protected | `13:1` protected envelope |
| `nested.sv` | `begin_protected` inside an open envelope | `3:1` and `11:1` protected envelope |
| `truncated.sv` | no `end_protected` before the end of the file | `13:1` unterminated protected envelope |
| `encryption_envelope.sv` | `pragma protect ... begin`/`end` around plaintext | `2:1` encryption envelope |
| `legacy_protected.v` | Verilog-XL `` `protected``/`` `endprotected`` (both editions) | `5:1` protected envelope `` `protected`` |
| `legacy_encryption.v` | Verilog-XL `` `protect``/`` `endprotect`` (both editions) | `4:1` encryption envelope `` `protect`` |
| `no_envelope.sv` / `.out` | `author`, `author_info`, `comment` outside any envelope | runs; `b=1` (hand-derived: `b` follows `a = 1`) |
