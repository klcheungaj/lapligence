# Backend handlers

Child modules share backend state/protocol types and consume owned results.
Blocking Slang work and filesystem isolation stay in scheduling/staging.

- `state`: owned root snapshots and synchronous queries/updates.
- `scheduling`: jobs, debounce, rescans, config reloads and watchers.
- `inputs`: the files the newest analysis of a root read, and the rules that
  decide from them whether an event starts another one.
- `identifiers`: the one table that answers a document's semantic-token,
  definition and outline requests, and the isolated parses behind it.
- `positions`: the texts of the open documents beside those that the committed
  analyses compiled, and the positions between them (`../../position_map.rs`). An
  unedited document shares its text with the analysis, so a pointer comparison tells
  it. Hover, references, definitions in other documents, symbols, the explorer and the
  token dump answer at the places of the current texts and leave out what has none;
  `prepareRename` and `rename` answer `ContentModified` for an edited document, because
  an edit elsewhere would change something else. A document is the file an analysis
  read by its path or by its identity (a symbolic link either way).
- `staging`: bounded inputs, private shadow paths and include authorization.
- `diagnostics`: commits, ticket-ordered publication, deduplication and shared-file
  aggregation. Digests and unions compare the places of the analysed texts; a payload
  moves to the current texts when it is sent (`positions`).
- `tests`: private backend regressions.
