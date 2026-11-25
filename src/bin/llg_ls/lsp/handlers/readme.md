# Backend handlers

Child modules share backend state/protocol types and consume owned results.
Blocking Slang work and filesystem isolation stay in scheduling/staging.

- `state`: owned root snapshots and synchronous queries/updates.
- `scheduling`: jobs, debounce, rescans, config reloads and watchers.
- `identifiers`: the one table that answers a document's semantic-token and
  definition requests; which positions of the committed analysis hold in the
  current text of a document (`answers_at`, `current_locations`). In an edited
  document that no commit has reached, hover, references and definitions name only
  positions whose identifier the text still has there; `prepareRename` and `rename`
  answer `ContentModified`, because an edit elsewhere would change something else.
- `staging`: bounded inputs, private shadow paths and include authorization.
- `diagnostics`: commits, ticket-ordered publication, deduplication and shared-file
  aggregation.
- `tests`: private backend regressions.
