# Native value assignments, links and conditionals (SIM-004)

The task module runs every positive fixture through the public CLI in both HDL
optimizer modes; `native_nba` and `native_chandle_nba` also run on legacy,
compact/portable and compact/GMP backends (GMP when `LLG_TEST_GMP_ROOT` is set),
and `native_nba` executes after the frontend snapshot and owned Db are
destroyed. Expected outputs are derived by hand from the cited clauses, not
captured from llg.

A nonblocking write to a persistent string or chandle (module, package or
static subroutine variable, or a member of a module record) queues one
pending-value record with an owned copy of the issue-time value; untimed and
delayed forms differ only in the commit time. See
`src/sim/rt/value/ownership.md`.

| Fixture | Clause and independent oracle |
| --- | --- |
| `native_nba` | IEEE 1800-2009 §§4.6, 4.9.4, 10.4.1-10.4.2, 6.16, 7.2. `s <= t` captures `src` before `t` changes, so the active region still shows `old changed` (1) and the NBA region installs `src`; the `always_comb` length reader then sees 3 (2). The value queued by the automatic task `issue` survives its return: `abc!` (3); a function result: `m-7` (4). Ordered NBAs to one target commit in issue order, the last being `{order, "c"}` with the issue-time `x`: `xc` (5). A swap reads both old values: `B A` (6). `late <= #2` commits after the untimed `now`: `now` then `late` (7, 8). `rb <= ra` keeps `x 3 1.50` although `ra.s` changes after issue, while the active region still shows the defaults `[] 0` (9, 10). A record function result: `fn 4 2.00` (11); member NBAs: `mem 9` (12). A pattern NBA commits `pat 5 0.25` one unit before `rb <= #1 rc` commits `mem 9` (13, 14). A posedge pipeline shifts strings like packed registers: `c1 c0`, then `c2 c1` (15, 16). |
| `native_chandle_nba` + `.c` | §§6.14, 10.4.1-10.4.2, 35.5.6. Chandles are borrowed foreign pointers: an untimed NBA changes `cur` from object 11 to 22 only in the NBA region (1, 2); `cur <= #2 h1` commits after `cur <= null` (3, 4); the automatic task's queued handle survives its return (5); a swap gives `22 11` (6). A record NBA keeps the issue-time chandle and tag although `ra` changes (7); a delayed record pattern commits two units later (8, 9). The blocking delayed record copy captures `ra` before suspending, so the forked tag change at +1 is not seen (10, 11). Releasing both objects leaves no live object and no bad handle (12). |
| `witness_string_nba`, `witness_chandle_nba`, `witness_string_record_nba` | Adopted FND-002 witnesses with quiet `$finish(0)`: `new`, `1`, `ok 7`. |

## Negatives

| Fixture | Boundary |
| --- | --- |
| `neg_string_element_nba` | §§6.16, 6.21: a string is dynamically sized, so its bytes are not nonblocking targets (llg diagnostic; the frontend accepts it). |
| `neg_class_string_nba` | §6.21: class properties are members of dynamic objects (llg diagnostic). |
| `neg_automatic_string_nba` | §§6.21, 10.4.2: automatic variables are not nonblocking targets (frontend). |
| `neg_automatic_monitor` | §§6.21, 21.2.3: the frontend rejects tracing an automatic variable; the monitor lifetime question stays unresolved. |
| `neg_chandle_continuous`, `neg_chandle_port` | §6.14: chandles are not continuous targets or ports, including a record port with a chandle member (frontend). Packed chandles and chandle arithmetic stay covered by `sim_003/neg_chandle_packed` and `neg_chandle_arithmetic`. |
| `neg_static_native_record_nba` | Legal by §10.4.2; a static subroutine native record root replaces its leaves on assignment, so a queued leaf pointer cannot be retained and the write is rejected explicitly. |
