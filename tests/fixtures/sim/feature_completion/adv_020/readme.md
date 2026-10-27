# ADV-020 fixtures: VPI symbol parity and multichannel descriptors

Each `<name>.sv` has a C application `<name>.c` that registers the system
tasks and functions the HDL calls. `tests/sim_feature_completion/adv_020.rs`
loads it as a run-time plugin (`LLG_VPI_PLUGIN`; on Windows a DLL linked
against a model import library) and, for `symbols`, also links it as a static
archive with `--dpi-lib` (every host, no import library), declares the calls
with `--define-system-task`, and compares stdout with `<name>.out` in both
optimizer modes on the legacy, compact/portable and compact/GMP value
backends. Clause numbers are IEEE 1800-2009; line numbers refer to
`SystemVerilog-1800-2009.txt`.

The expected outputs were derived by hand from the rules below and then
checked against a run; no line was accepted only because the run printed it.

Every check reads the `vpi_chk_error` level of the routine it tests before
any other VPI call, because 38.2 (L61808) says the error "status shall be
reset by any VPI routine call except vpi_chk_error()". Level 3 is `vpiError`.

## symbols (A01 groundwork)

`symbols.c` builds a table with the address of each of the 41 routines of the
Annex K `vpi_user.h` listing (L75839-L75974, from `vpi_register_cb` to
`vpi_handle_by_multi_index`) and of `vpi_register_assertion_cb`, the routine
the Annex M `sv_vpi_user.h` listing declares (L76835). Taking an address makes
the loader resolve the symbol when the plugin loads, and makes the static link
resolve it, so the first line `routines: 42 of 42 resolved` is printed only
when the model defines every routine.

The routines llg does not provide yet return their failure value and leave
level 3 with code `LLG_VPI_UNSUPPORTED` and a message naming the routine:

- `vpi_handle_multi` returns NULL (38.22: "Returns: vpiHandle Handle to an
  object.", L63405; llg has no `vpiPortInst`/`vpiInterModPath` objects).
- `vpi_get_delays`/`vpi_put_delays` return nothing; the application's delay
  record and its `da` array are left as they were (`da=same no_of_delays=1
  time_type=2 low=77`, where 2 is `vpiSimTime`).
- `vpi_get_data` returns 0 (38.9, L62169-L62170: "The return value shall be
  the number of bytes retrieved. On a failure, the return value shall be 0.")
  and `vpi_put_data` returns 0 (38.31, L63792-L63793: "The return value shall
  be the number of bytes written. A zero shall be returned if an error is
  detected."); the buffer still reads `abc`.
- `vpi_register_assertion_cb` returns NULL and never calls the callback.

The last line shows that the next routine clears the status (38.2): `vpi_get`
of `tb` returns `vpiModule` with level 0.

## mcd (38.24-38.29)

The design opens `adv020_hdl.txt` with `$fopen` as a multichannel descriptor
`m` and `adv020_fd.txt` as a file descriptor `fd` (bit 31 set), writes
`hdl 1` to `m`, then calls `$mcd(m, fd)`. Descriptors are shared with the
built-in tasks (38.27, L63632-L63635: "The mcd descriptor returned by
vpi_mcd_open() routine is compatible with the mcd descriptors returned from
the $fopen system function. The mcd descriptors returned from vpi_mcd_open()
and from $fopen may be shared between the built-in system tasks that use mcd
descriptors and the VPI routines that use mcd descriptors."). The exact bit
values are not printed; the checks print relations between them.

- **open.** `vpi_mcd_open("adv020_v1.txt")` returns a single new channel `a`
  (one bit, not bit 0, not bit 31; 21.3.1, L36606-L36607: "a single bit is
  set indicating which file is opened. The least significant bit (bit 0) of an
  mcd always refers to the standard output."). Opening the same name again
  returns `a`, and opening the name `$fopen` gave `m` returns `m` (38.27,
  L63639-L63641: "If the file has already been opened either by a previous
  call to vpi_mcd_open() or using $fopen in the SystemVerilog source code,
  then vpi_mcd_open() shall return the descriptor number."). The file open
  only as the descriptor `fd` returns 0 with level 3, because a file
  descriptor is not an mcd (decision A20-D1). NULL, the empty name and a path
  in a missing directory return 0 with level 3 ("shall return a 0 on error",
  L63639).
- **name.** `vpi_mcd_name` returns the name each file was opened with for `a`,
  `m` and `fd` (38.26, L63584-L63589: the name "of a file represented by a
  single-channel descriptor, cd ... The channel descriptor cd could be an fd
  file descriptor returned from $fopen ... or an mcd"), `stdout` for channel 1
  and `stderr` for the preopened descriptor `0x80000002` (21.3.1). Descriptor
  0, the two-channel `a|m`, an unopened channel and an unopened file
  descriptor are not single open files: NULL with level 3 ("On error, the
  routine shall return NULL.", L63585). The result is one buffer that the
  next call overwrites ("This routine shall overwrite the returned value on
  subsequent calls.", L63585-L63586): both pointers are equal and read the
  second name.
- **printf.** The routines return the number of characters of the formatted
  text (38.28, L63686: "The routine shall return the number of characters
  printed or return EOF if an error occurred."), once however many channels
  receive it: `to a\n` is 5, `to a and m\n` 11, `to channel 1\n` 13,
  `to channel 1 and a\n` 19, `partial\n` 8, the 700-character line 700,
  `v=abc 7\n` (through `vpi_mcd_vprintf`, 38.29) 8, the empty text 0.
  Channel 1 is the standard output, so `to channel 1` and
  `to channel 1 and a` appear in stdout between the report lines. Descriptor
  0, an unopened channel and `fd` print nothing and return EOF (-1) with
  level 3 (38.28, L63678-L63679: "vpi_mcd_printf() shall not write to a file
  represented by an fd file descriptor"); a NULL format is EOF with level 3.
  `a|unopened` writes `partial` to `a`, returns 8 and leaves level 3 for the
  unopened channel, as `$fdisplay` reaches the open channels of a descriptor
  and reports the others (decision S25-D7).
- **flush.** 0 for `a` and `1|a` (38.25, L63543: "0 if successful; nonzero if
  unsuccessful."); 1 with level 3 for descriptor 0, an unopened channel,
  `a|unopened` and `fd`.
- **close.** 38.24 (L63515-L63521): "On success, this routine shall return a
  0; on error, it shall return the mcd value of the unclosed channels. ...
  descriptor 1 is for the output channel of the tool that invoked the PLI
  application and the current log file". So `vpi_mcd_close(1)` returns 1 with
  level 3 and channel 1 still prints (`channel 1 is still open`, 24
  characters). Descriptor 0 closes nothing and returns 0 with level 3. An
  unopened channel and `fd` come back unclosed with level 3 (A20-D1); `fd`
  stays open, so the design's later `$fdisplay(fd, "fd still open")` reaches
  its file. `b|unopened` closes `b` (its line was written before) and returns
  the unopened bit; afterwards `b` has no name, does not print (EOF), comes
  back from a second close, and `1|b` comes back whole. A new
  `vpi_mcd_open` reuses `b`'s freed channel (llg allocates the lowest free
  channel) and closes with 0.
- **after `$fclose`.** `$after_close(m)` runs after the design wrote `hdl 2`
  and closed `m` and `fd`: `m` has no name and comes back from
  `vpi_mcd_close` with level 3; closing `a` succeeds with 0.
- **HDL on a VPI channel.** `$vopen` returns `vpi_mcd_open("adv020_v4.txt")`,
  the design writes to it with `$fdisplay` and closes it with `$fclose`.
- **files.** `$show_files` prints the files: `adv020_v1.txt` has the lines
  written to `a` in call order (the 700-character line summarized);
  `adv020_hdl.txt` has `hdl 1`, then `to a and m` from the VPI write, then
  `hdl 2`, because both sides write to the same channel in call order;
  `adv020_fd.txt` has the line the design wrote after `vpi_mcd_close(fd)`
  refused it; `adv020_v2.txt` has the line written before it closed;
  `adv020_v3.txt` is empty; `adv020_v4.txt` has the design's line.

## value_array (38.16, 38.35)

`value_array.c` (`$va_get`, then `$va_put`) reads and writes sections of the
design's unpacked arrays; the design prints the arrays after the puts.

- **Order.** 38.16 (L63066-L63071): "The array element values are retrieved
  consecutively in order of the fastest varying index (rightmost unpacked
  range of the array declaration), followed by more slowly varying indices
  accordingly until the number of elements (num) has been retrieved. Index
  values within each range are ordered from leftmost range value to
  rightmost. For example, elements of an array a[2:0][3:5] with index_p[0] =
  1 and index_p[1] = 4 would be retrieved in the order a[1][4], a[1][5],
  a[0][3], a[0][4], a[0][5]". The first get is that example: the five
  elements are `0x14`, `0x15` with bits 7:4 set to `z1x0` (A byte `65`, B
  byte `a0`), `0x03`, `0x04` and all-x, each with bits 41:40 set (top byte
  `03`). `iv [1:0]` from index 1 reads `iv[1]` then `iv[0]`.
- **Raw formats.** 38.16 (L62999-L63006): "Each array element occupies
  ngroups*2 bytes stored consecutively as A/B byte groups ... ngroups is
  computed given the array element size in bits (= elemBits) as follows: int
  ngroups = (elemBits + 7) / 8;" and (L63055-L63058) "the lsb of rawvals[0]
  and rawvals[ngroups] indicates the A and B value of the lsb (0th) bit of
  the first array element". The lines print each element's bytes most
  significant first. `vpiRawTwoStateVal` omits the B bytes (L63009-L63012);
  an x or z bit of the four-state `a` reads as 0 there (`65` becomes `45`,
  decision A20-D6).
- **Buffers.** 38.16 (L63073-L63079): "By default, array values shall be
  returned in memory allocated by VPI ... However, if the application sets the
  vpiUserAllocFlag in the value.flags field, the function will assume the
  calling application has set the value field to point to a buffer of
  sufficient size". With the flag the value pointer stays the application's
  buffer (`buffer=same`); without it the values are in simulator storage.
- **Formats.** `vpiVectorVal` gives `((elemBits + 31) / 32)` A/B word pairs
  per element (L63036-L63042; printed high word first); `vpiIntVal` the low 32
  bits with the element's sign (`b` is unsigned: 2049, 255, 291, 4095);
  `vpiShortIntVal` of byte and shortint elements and `vpiLongIntVal` of byte,
  shortint and longint elements, sign-extended (L63013-L63018); `vpiRealVal`
  and `vpiShortRealVal` of real and shortreal arrays; `vpiTimeVal` as
  `vpiSimTime` (type 2) high and low words (5000000000 is high 1, low
  705032704); the net array `n` reads its resolved values (`a/0` and `9/c` for
  `4'bxz01`).
- **Get errors.** Each leaves level 3 and a NULL value pointer (L63102-L63103:
  "If vpi_get_value_array() returns NULL for the value pointer in either case,
  it shall indicate that a VPI error has occurred"): a handle that is not an
  unpacked array (`tb.w`, the element `tb.a[0][3]`); an array of unpacked
  structures (L62951-L62952: "The elements of arrays are not allowed to be of
  an unpacked type themselves"); an index outside its range, a count of 0, a
  section past the end and a NULL `index_p` (decision A20-D5); a flag other
  than `vpiUserAllocFlag` (A20-D5); `vpiUserAllocFlag` with a NULL buffer;
  `vpiBinStrVal` (L63044: "All other formats not mentioned here are
  unsupported, and shall result in an error if requested"); and formats the
  text does not list for the element type (L63045-L63046): `vpiShortIntVal`
  or `vpiLongIntVal` of int elements, `vpiRealVal` of bit elements,
  `vpiShortRealVal` and `vpiIntVal` of real elements. A NULL `arrayvalue_p` is
  level 3.
- **Puts.** 38.35 (L64504-L64551) example: the five elements from `a[1][4]`
  take one to five low bytes of 1, the last with an x low byte, with
  `vpiPropagateOff` (`00000000001`, `00000000101`, `00000010101`,
  `00001010101`, `001010101xx`). `vpiOneValue` (L64476-L64477: "This flag set
  causes the function to apply only a single element value to the entire
  array section specified") writes `abc` to all of `b`; `b[1]` changed, so the
  `always @(b[1])` process ran once although `vpiPropagateOff` was set
  (`changes=1`, decision A20-D7). L64449-L64450: "If the vpiRawFourStateVal
  format is set for a 2-state array type, the bvalbits shall be ignored. If
  the vpiRawTwoStateVal format is specified for a 4-state array type, the
  bvalbits shall be assumed to be 0.": `b[2]` becomes `234`; `a[2][3]`
  becomes `201` (byte 5, bit 47, lies above bit 41). `vpiShortIntVal` puts to
  int elements (L64414-L64415: "appropriate only for arrays of
  vpiShortIntVar, vpiIntVar or vpiLongIntVar elements"): `iv[1]=-1`,
  `iv[0]=300`; `vpiLongIntVal` to longint, `vpiRealVal`, `vpiShortRealVal`,
  `vpiTimeVal` (high 1 is 4294967296) and `vpiVectorVal` (`ig[1]` takes a z
  low nibble) write their elements.
- **Put errors.** Level 3 and no element written: the net array `n` (decision
  A20-D8), the flags `vpiUserAllocFlag` and `vpiInertialDelay` (L64482:
  "Flags other than vpiPropagateOff, vpiOneValue, or vpiNoDelay (the default)
  specified shall be an error."), a NULL buffer, a section past the end (`b`
  still ends `234 abc`), `vpiShortIntVal` to byte elements, `vpiLongIntVal` to
  shortint elements (L64417-L64418: "appropriate for arrays of vpiLongIntVar
  elements"), an array of unpacked structures, a raw format to a real array
  and a NULL `arrayvalue_p`.

## Decisions

The choices where the text is silent are the portable cases A20-D1 to A20-D4
in `tests/fixtures/sim/lrm_decisions/` (`docs/lrm_decisions.md`): file
descriptors given to the mcd routines (A20-D1), the names of the standard
streams (A20-D2, `stdout` for channel 1 and `0x80000001`, `stdin` and `stderr`
for `0x80000000` and `0x80000002`), the count and status of a write to several
or partly closed channels (A20-D3), and closing or flushing channels that are
not open (A20-D4). The value-array choices are A20-D5 (argument errors, get
flags, no partial writes), A20-D6 (x and z read as 0 in the raw two-state
form), A20-D7 (`vpiPropagateOff` still propagates) and A20-D8 (no puts to net
arrays).
