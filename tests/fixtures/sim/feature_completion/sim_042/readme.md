# SIM-042 fixtures: VPI object and value access

Each `<name>.sv` has a C application `<name>.c` that registers the system
tasks and functions the HDL calls. `tests/sim_feature_completion/sim_042.rs`
loads it as a run-time plugin (`LLG_VPI_PLUGIN`; on Windows a DLL linked
against a model import library, exporting `vlog_startup_routines` through
`vpi_user.h`'s `PLI_DLLESPEC` as in Annex K, `SystemVerilog-1800-2009.txt`
L75978: "PLI_VEXTERN PLI_DLLESPEC void (*vlog_startup_routines[])();") or links it as a static archive
with `--dpi-lib` (every host), declares the
calls with `--define-system-task`, and compares stdout with `<name>.out` in
both optimizer modes on the legacy, compact/portable and compact/GMP value
backends. Clause numbers are IEEE 1800-2009; line numbers refer to
`SystemVerilog-1800-2009.txt`.

The expected outputs were derived by hand from the rules below and then
checked against a run; no line was accepted only because the run printed it.

## objects (A01)

`$walk` runs in the named block `tb.main`, so `vpiScope` of the call is
`tb.main` (`vpiNamedBegin`) and `vpiModule` is `tb` (37.3).

- **Hierarchy.** `vpi_iterate(vpiModule, NULL)` returns the one top `tb`
  (`vpiTopModule` 1). Children are returned in catalog order, which is the
  order of their names (S42-D12): `u1`; the internal scopes `gen[0]`,
  `gen[1]` (generate block names per 27.4) and `main`; the variables `b`
  (`vpiByteVar`), `bv` (`vpiBitVar`), `grid` and `mem` (`vpiRegArray`, size =
  element count, 37.17 detail 9), `i` (`vpiIntVar`), `ii` (`vpiIntegerVar`),
  `r` (`vpiRealVar`, `vpiSize` 64, S42-D8), `t` (`vpiTimeVar`), `up` and `v`
  (`vpiReg`); the net `w`.
- **Parameters.** `L = W * 2 = 8` is a localparam (`vpiLocalParam` 1); `P`
  keeps its four-state value `1010x1z0`, whose decimal string is `X` (some
  bits unknown, S42-D6). Integer parameters are 32 bits.
- **Ports** are returned by `vpiPortIndex`: `a` input (1), `y` output (2),
  both 4 bits; `vpiLowConn` is the port's internal object `tb.u1.a`/`tb.u1.y`.
- **Lookup.** `vpi_handle_by_name("gv", gen[1])` finds `tb.gen[1].gv`
  (relative to the scope, 38.21); `"v"` relative to `u1` is not found because a
  scoped lookup searches only that scope, and the miss sets no error status
  (S42-D11).
- **Tool information.** `vpi_get_vlog_info` returns 1 with the model's
  command line (`argv[0]` set) and the product name `lapligence` (38.17).
- **Values.** `up` is `logic [0:3]` = `10xz`: `vpiIntVal` reads X/Z bits as 0
  (38.15, Table 38-3) so it is 8; its hex digit has an x and a z bit, so it is
  `X`. `vpiObjTypeVal` gives `vpiVectorVal` (9) for vectors, `vpiIntVal` (6)
  for `int`/`byte`/`integer`, `vpiRealVal` (7) for real and `vpiTimeVal` (11)
  for time (38.15). `ii` is all X: binary all `x`, decimal `x`, integer 0.
  `r = 1.25` reads as integer 1 (rounded).
- **Bounds.** `vpiLeftRange`/`vpiRightRange` are `[7:0]` for `v`, `[0:3]` for
  `up`, `[5:2]` for `bv`, `[0:3]` for `mem` and `[5:2]` for the part-select
  `v[5:2]`; `grid[1:0][2:0]` iterates two `vpiRange` objects.
- **Elements.** `mem[2] = 1x0z_0000` is a `vpiReg` with `vpiArrayMember` 1,
  name `mem[2]`, `vpiIndex` 2, hex `X0` and decimal `X`; index 4 is outside
  `[0:3]` and returns NULL with an error (S42-D7). `grid[0][1]` by
  `vpi_handle_by_multi_index` is -1; `tb.grid[1][2]` by name is 3 and is a
  different object (`vpi_compare_objects` 0).
- **Selects.** `v[5:2]` of `8'h3c` is `1111`; `v[0]` is a `vpiRegBit` 0; bit
  2 of `up` (`[0:3]`) is `x`; `vpi_iterate(vpiBit, v)` returns the bits from
  the left index: `0 0 1 1 1 1 0 0`. A bit of the net `w` is a `vpiNetBit`;
  `mem[1][5]` is bit 5 of `8'h22` = 1.
- **Writes.** `mem[3] := 'ha5`; `v[5:2] := 1010` gives `v = 8'h28`; `up[3]
  := 1` gives `10x1`; `grid[1][0] := 77`; `r := 3.5`; `i := "-123"`; the
  two-state `bv := 0x01` stores `0001` (X becomes 0, 6.11.1); `local_v :=
  3'b010` through `vpiVectorVal`; `gen[1].gv := 2`. One time step later `w =
  ~v[3:0] = 4'h7`.

## values

- **Formats (Table 38-3).** `v = 1x0z_0101_zzzz`: octal digits from the
  least significant group are `zzz` → `z`, `z10`/`01z` mixed → `Z`, `1x0` →
  `X`, so `XZZz`; hex `X5z`; decimal `X`; integer `0x850 = 2128`. All-x `xs`
  gives `x` everywhere and integer 0; all-z `zs` gives `z`. `pz = 10z1` gives
  octal `1Z`, hex `Z`, decimal `Z` (S42-D6) and integer 9. `s = -300` is
  `fed4`, octal `177324`, real -300. The 72-bit `wide` vector has three
  `aval/bval` words, the top one `00000081/00000000`. `"Hi!"` in 32 bits reads
  back as the string `Hi!` (leading zero bytes skipped). A real object rejects
  `vpiBinStrVal` (S42-D9) and rounds -2.5 to -3 for `vpiIntVal`.
  `vpiStrengthVal` reports strong (0x40) for 0/1 and high impedance (0x01) for
  z (38.15).
- **Puts.** String `"OK"`, octal `7_7` (underscores ignored), hex `z`
  (fills the value), decimal `-1` and binary `10xz` (zero-extended) are
  accepted; every `vpiNoDelay` put returns NULL (S42-D1). A string to a real
  and `vpiStrengthVal` to a vector are illegal (38.34) and a scalar to a
  vector is an error (S42-D13). The real 6.5 put to `s` rounds to 7.
- **Delays (38.34).** At time 0: inertial 1@2, then inertial 2@3 removes it;
  transport 4@6, then transport 5@5 removes the later 4@6; pure transport 6@4
  with `vpiReturnEvent` returns a `vpiSchedEvent` that is scheduled; 7@7 is
  cancelled (which frees its handle, so a later `vpi_get` reports a stale
  handle); a scaled-real delay of 8.0 time units lands at 8. `d` therefore
  changes at 3 (2), 4 (6), 5 (5) and 8 (8). A delayed put without a time is
  an error. At 10 the event has occurred (`vpiScheduled` 0) and releasing the
  handle succeeds.
- **Force and release (10.6.2, 38.34).** At 10, `f` is forced to `aa` and
  the net `n` to `3`. The HDL writes `f = 11` and `d = 9` are overridden. On
  release the variable `f` keeps `aa` until its next assignment, the net `n`
  is recomputed from its driver (`9`), and `value_p` returns those values.

## systf (A02)

- `$sum` is a `vpiSizedSignedFunc`; its compiletf checks two arguments for
  each of the two call sites and its sizetf runs once (36.8.1), giving 12
  bits for every site (S42-D4). `5 + 10 = 15`, `5 + (-7) = -2`.
- `$noret` puts no value: the result is 0 (38.34, S42-D3).
- `$half` is a `vpiRealFunc`; a `vpiStringVal` put to it is illegal (38.34)
  and the real put 0.25 is returned.
- `$big` is a `vpiTimeFunc` (64 bits): `2 << 32 | 5 = 8589934597`.
- `$noop` registers with NULL calltf and compiletf, which is legal (38.37).
- `$args`: `v` is the variable itself (writable, `v := 9`), `mem[1]` is the
  array element (writable, `mem[1] := 4`), `v[3:0]` a `vpiPartSelect`,
  `3'b1x0` a `vpiConstant` of `vpiBinaryConst` (3), `"txt"` a `vpiStringConst`
  (6), `r` the real variable, and `v + 8'd1` a `vpiOperation` value snapshot.
  Puts to constants and operations are errors.
- `$count` stores user data per call site in compiletf (38.33); the call site
  in the loop counts 1, 2 and the second site 1 (S42-D10).
- `$late_put` puts to the `$noret` call handle while that call is not active:
  the put is ignored without an error (38.34). Reading the value of an
  inactive call is an error. compiletf ran 5 times (two `$sum`, one `$noret`,
  two `$count`) and sizetf once.

## errors and missing (A03)

Every refused operation returns NULL, `vpiUndefined` or 0 and sets the error
status; `vpi_chk_error` returns the level `vpiError` (3), and a second call
returns the same level because `vpi_chk_error` does not reset it (38.2,
S42-D2). An empty but supported iteration (no nets, no submodules in `tb`)
returns NULL without an error (38.23), and so do a name lookup that finds
no object (S42-D11), a bit index outside a vector's range (S42-D7) and
`vpiSize` of a real, which is 64 (S42-D8); a part-select written in reverse
order inside a name is still an error. Released handles, handles of a
finished iterator (38.40), a reused slot's old handle and argument handles of
a returned call are stale; releasing a catalog object is accepted and leaves
it valid. The call handle persists between calls (38.14). `$stress` makes
20000 rounds of element, bit and iterator handles and probes each released
handle afterwards (the sanitizer lane runs it under ASan/UBSan).

`missing.sv` calls `$absent`, which the compiler knows but `errors.c` never
registers: the model stops before time 0 with
`llg: VPI error: no VPI registration for `$absent` (called at <path>/missing.sv:6)`
and exit status 1.
