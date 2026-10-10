# SIM-028 object, process and shuffle random streams

IEEE 1800-2009 §§18.13-18.15, 7.12.2, 8.11, 20.15 and Annex N, and IEEE
1364-2001 §17.9, are the oracles. Line numbers refer to the `pdftotext
-layout` extractions `SystemVerilog-1800-2009.txt` (SV) and
`Verilog-1364-2001.txt` (V2001). The suite
`tests/sim_feature_completion/sim_028.rs` runs every positive fixture in both
optimizer modes on the legacy, compact/portable and compact/GMP value
backends.

The generator behind `$urandom`, `$urandom_range`, process and object streams
and `shuffle` is implementation dependent, so those fixtures check relations
only: equal seeds give equal sequences, a restored state replays, a shuffle is
a permutation, unrelated activity leaves a stream unchanged. `$random` and the
`$dist_*` functions are fixed by Annex N, so their expected values are
numbers, produced by the independent transcription `annex_n_reference.c` and
never captured from llg.

## Clauses relied on

- SV 18.13.1 L31018: "function int unsigned $urandom [ (int seed ) ] ;" and
  L31021-31022: "The RNG shall generate the same sequence of random numbers
  every time the same seed is used."
- SV 18.13.2 L31044-31045: "function int unsigned $urandom_range( int
  unsigned maxval, int unsigned minval = 0 );".
- SV 18.13.3 L31086: "function void srandom( int seed );" and L31089: "The
  srandom() method initializes an object's RNG using the value of the given
  seed."
- SV 18.13.4 L31100-31104: "The get_randstate() method returns a copy of the
  internal state of the RNG associated with the given object. The RNG state is
  a string of unspecified length and format."
- SV 18.13.5 L31115: "The set_randstate() method copies the given state into
  the internal state of an object's RNG." L31117-31119: "Calling
  set_randstate() with a string value that was not obtained from
  get_randstate(), or from a different implementation of get_randstate(), is
  undefined."
- SV 18.14.1 L31148-31151: "Each module instance, interface instance, program
  instance, and package has an initialization RNG. ... An initialization RNG
  shall be used in the creation of static processes and static initializers".
- SV 18.14.1 L31152-31160: "Each thread has an independent RNG for all
  randomization system calls invoked from that thread. When a new dynamic
  thread is created, its RNG is seeded with the next random value from its
  parent thread. ... When a static process is created, its RNG is seeded with
  the next value from the initialization RNG of the module instance ...
  containing the thread declaration. ... When adding new threads to an
  existing test, they can be added at the end of a code block in order to
  maintain random number stability of previously created work."
- SV 18.14.1 L31161-31169: "Each class instance (object) has an independent
  RNG for all randomization methods in the class. When an object is created
  using new, its RNG is seeded with the next random value from the thread that
  creates the object. When a class object is created by a static declaration
  initializer ... the RNG of the created object is seeded with the next random
  value of the initialization RNG of the module instance ... Object stability
  shall be preserved when object and thread creation and random number
  generation are done in the same order as before."
- SV 18.14.3 L31241: "Objects can be seeded at any time using the srandom()
  method." SV 18.15 L31270: "Each object maintains its own internal RNG, which is used
  exclusively by its randomize() method."
- SV 8.11 L9990-9998: in a shallow copy "All class properties, including the
  internal states used for randomization and coverage are copied to the new
  object. ... The internal states for randomization include the random number
  generator (RNG) state".
- SV 7.12.2 L9366: "shuffle() randomizes the order of the elements in the
  array."
- SV 6.11.2 L5489-5490: "When a 4-state value is automatically converted to a
  2-state value, any unknown or high-impedance bits shall be converted to
  zeros."
- SV 6.12.2 L5531: "Real numbers shall be converted to integers by rounding
  the real number to the nearest integer".
- SV 20.15 L35286-35288: "The value generation algorithm for these system
  functions is part of this standard, ensuring repeatable random value sets
  across different implementations. The C source code for this algorithm is
  included in Annex N."
- SV 20.15.1 L35315: "The seed argument shall be an integral variable."
- SV 20.15.2 L35348-35349: "For the exponential, poisson, chi-square, t, and
  erlang functions, the arguments mean, degree_of_freedom, and k_stage shall be
  greater than 0." L35355-35356: "the seed argument is an inout argument; that
  is, a value is passed to the function, and a different value is returned."
- SV Annex N Table N.1 L76887: "$random rtl_dist_uniform (seed, LONG_MIN,
  LONG_MAX)"; N.2 gives the C source; V2001 17.9.3 gives the same source.
- V2001 17.9.1 L20375: "The seed parameter shall be either a reg, an integer, or a
  time variable."

## Fixtures

| Fixture | Acceptance | Oracle |
| --- | --- | --- |
| `annex_n` | A03 | 79 vectors of `$random` and all seven `$dist_*` functions, including `start >= end` and nonpositive mean/degree of freedom/k-stage (which return 0 and leave the seed, N.2), each with the seed written back. `annex_n.out` is the stdout of `annex_n_reference.c`; the suite rebuilds that program with the host C compiler at `-O0` and `-O2` and compares. |
| `annex_n_reference.c` | A03 | Transcription of SV Annex N.2 with the adaptations listed in its header (32-bit `long`, wrap-around multiply in unsigned arithmetic, range-checked conversions, the listing's unbalanced parenthesis). |
| `seed_aliasing` | A03 | Seeds reached through `ref`, `inout`, an array element, a class property, a seed formal aliased with the result, `time`, 48-bit, signed and unsigned 16-bit variables, X/Z seeds and one seed shared by successive calls. Values are reference vectors (seed 1, -1, 0, 7) under decisions S28-D2/S28-D3. |
| `object_streams` | A02, A01 | Object `srandom`/`get_randstate`/`set_randstate` on classes with and without properties, inherited and implicit-`this` calls, array elements, independence from thread draws, hierarchical object seeding, the one value creation takes, 2-state seed conversion and shallow copies. |
| `state_replay` | A02 | `get_randstate`/`set_randstate` of the running thread replays `$urandom`, `$urandom_range`, `shuffle` and fork-child seeding; through a process handle it replays another process's draws; X/Z arguments of `srandom`, `$urandom` and `$urandom_range` read as 0 and real arguments round. |
| `shuffle_streams` | A02 | Permutations of queue, dynamic, fixed, string, real and class-handle arrays, empty and single-element shuffles; reseeding the calling thread reproduces the order and the following draws; a child thread's shuffle leaves the parent alone; a method body shuffles from the thread, not its object. |
| `thread_stability` | A01 | No `.out`: the suite runs it with and without `-D EXTRA_ACTIVITY` on every backend and optimizer mode and requires identical output apart from `extra` lines. The extra activity is a new instance before and after `tb`'s own, a display, and a thread added after `tb`'s existing processes that creates and seeds an object, shuffles, draws and forks. It also checks that two identical instances draw different values (S28-D7). |
| `neg_srandom_override` | nearest illegal | A class declaring `srandom` is a compile error ("cannot override built-in method"). |
| `neg_randstate_invalid` | boundary | A state string not produced by `get_randstate` is undefined (18.13.5); llg reports `invalid randstate string`, keeps the stream and exits with status 1 (S28-D9). |
| `neg_srandom_null` | boundary | `srandom` through a null handle is a null object access (8.4), a run-time error. |

Portable decision cases S28-D1 to S28-D8 are in
[`../../lrm_decisions/`](../../lrm_decisions/) and listed in
[docs/lrm_decisions.md](../../../../../docs/lrm_decisions.md).
