# LSP backend

`lsp.rs` connects tower-lsp requests to workspace/configuration state, bounded
admission, private shadow staging, analysis scheduling and diagnostics. Blocking
Slang work uses admitted buffers; async requests use owned `Analysis`.

`handlers.rs` defines protocol types, custom requests and `LanguageServer`.
[Handler domains](handlers/readme.md) own state, scheduling, staging and diagnostic
publication; `handlers/tests.rs` contains private unit/async regressions.
