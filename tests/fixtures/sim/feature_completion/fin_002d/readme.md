# FIN-002 allocation failure matrix (step 4)

One small model per runtime subsystem, run by
[`tests/sim_feature_completion/fin_002d.rs`](../../../../sim_feature_completion/fin_002d.rs):
each model is built once per optimizer mode with the allocation fault
injection and run once per failed allocation request; the table above the
tests names the defined result of every row. Method, environment variables and the list of what the
injection does not cover: [tests readme](../../../../readme.md#allocation-fault-injection).
The other rows of the failure matrix are in [`fin_002b`](../fin_002b/readme.md).

Every `.out` is the transcript of the fault-free run. For the ten
SystemVerilog-only fixtures it is the industry-standard behaviour (the same
source, the same plusargs); nothing in them depends on a random value, a
process order the standard leaves open or a tool-specific message.

| Fixture | Subsystem and what it does |
| --- | --- |
| `alloc_values.sv` | Packed values of one word, two words and heap words: arithmetic, shifts, x/z bits, signed decimal formatting, `real` conversions, an unpacked array; 400,000-bit operands so that the multiplication and the division take scratch from the heap inside the GMP kernels. |
| `alloc_strings.sv` | String concatenation, replication, methods, `$sformatf`/`$sformat`, conversions to and from packed values, a sorted queue of strings. |
| `alloc_containers.sv` | Dynamic arrays, queues and associative arrays (integral, string and wide keys), their growth, copies, slices, locator and ordering methods, nested forms, a queue of unpacked structures, streaming. |
| `alloc_classes.sv` | Class objects with strings, queues, dynamic and associative arrays, virtual methods, a parameterized registry, and 40 rounds of 12-object rings that only refer to each other, which the collector reclaims while the model keeps allocating. |
| `alloc_processes.sv` | A 31-deep recursive timed task, `fork` with `join`, `join_any`, `join_none`, `wait fork`, `disable fork`, a named-block `disable`, process handles with `kill`, `await` and `status`. |
| `alloc_sync.sv` | A bounded typed mailbox, a mailbox of structures, an untyped mailbox, a semaphore with six competing processes, named events and an array of events. |
| `alloc_random.sv` | Class randomization with `inside`, `dist`, `solve before`, implication, `foreach`, a dynamic array size, `randc`, an inline `with`, an unsatisfiable call, `constraint_mode`/`rand_mode`, `std::randomize`, `randcase`, `randsequence`. Only facts every solution satisfies are printed. |
| `alloc_coverage.sv` | A clocked covergroup with value bins, transition bins and a cross, a class covergroup with `with function sample`, the coverage queries, a coverage database written at the end of the run, an `assert property` and a `cover property`. |
| `alloc_files.sv` | Descriptors and a multichannel descriptor, formatted writes, `$readmemh`, `$fgets`, `$fscanf`, `$fgetc`/`$ungetc`, `$fread`, `$fseek`/`$rewind`/`$ftell`, `$writememh`/`$writememb`, `$sformat`/`$sscanf`, `$value$plusargs` and `$test$plusargs` (run with `+ratio=0.75 +name=llg +wide=123456789abcdef +flag`). |
| `alloc_waves.sv` | `$dumpfile`/`$dumpvars` over two parameterized instances with scalars, vectors, a 100-bit vector, a real, a tri-state net and unpacked arrays; `$dumpoff`, `$dumpon`, `$dumpflush`, `$dumpall`. |
| `alloc_foreign.sv`, `alloc_foreign.c` | DPI-C imports with an open array, strings, a 100-bit `logic` vector, a `chandle` and a context task that calls an exported function and a time-consuming exported task; a VPI application with four system tasks (`$inspect`, `$watch`, `$poke`, `$later`) that walks the variables of a scope, reads values as strings and vectors, writes values, and registers value-change, delay and end-of-simulation callbacks. |

### `alloc_foreign.out`

The expected lines follow from the two source files:

| Lines | Why |
| --- | --- |
| `sum 165` | 0 + 11 + 22 + 33 + 44 + 55. |
| `join left+vpi text!` | `c_join` joins its arguments with `+`. |
| `wide 9abcdef012345678900000001` | `c_wide` reverses the four 32-bit words of `100'h1_23456789_abcdef01_23456789` and keeps four bits of the new top word. |
| `note tb 1` .. `3`, `walk 6 at 25 close 7` | `c_walk(3)` reports its scope and waits one rising clock edge per step (edges at 5, 15, 25); `c_open("session")` adds the name's seven characters. |
| `inspect ...` | The `logic` and `string` variables of `tb` in declaration order (hexadecimal, and for the 100-bit ones the low vector word and the decimal string, which is right-justified in the 31 digits of the largest 100-bit value: decision S42-D6); `tb.text` has the 8 characters of its value; 10 variables in all. |
| `inspect tb.counter 8 02`, `watch counter 5` | The initial procedure resumes at the edge of time 25 before that edge's nonblocking update, so `counter` still reads 2; the third change after `$watch` is the update to 5 (3 at 25, 4 at 35, 5 at 45). |
| `poked 77 1234` | Four more edges after `$watch` (35 to 65), then `$poke` writes 77 and 4660 (`16'h1234`). |
| `later at 73 after 5 changes` | `$later(7)` at 66; `counter` changed at 25, 35, 45, 55 and 65. |
| `end of simulation after 7 changes` | Two more edges (75, 85) before `$finish` at 86; the end-of-simulation callback prints last. |
