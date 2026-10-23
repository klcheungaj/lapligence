# SIM-022 native initialization, package state and let environments

IEEE 1800-2009 is the oracle; Verilog-2001 keeps its own declaration
initialization (V 6.2.1). Line numbers refer to the `pdftotext -layout`
extractions `SystemVerilog-1800-2009.txt` (SV) and `Verilog-1364-2001.txt`
(V). The suite `tests/sim_feature_completion/sim_022.rs` runs every positive
fixture in both optimizer modes on the legacy, compact/portable and
compact/GMP value backends; `static_native_locals` also runs at
`--model-opt-level O0` and `O3` and is generated after the owned database is
dropped. Negative fixtures must stop in both optimizer modes with the quoted
located diagnostic and no simulation output.

The decisions this task made are S22-D1 to S22-D7 in
[lrm_decisions](../../../../../docs/lrm_decisions.md#native-initialization-and-let-environments-sim-022),
with portable cases under `tests/fixtures/sim/lrm_decisions/`.

## Clauses relied on

- SV 6.8 L5286-5287 (repeated in 10.5 L13286-13287): "Setting the initial
  value of a static variable as part of the variable declaration (including
  static class members) shall occur before any initial or always procedures
  are started". `init_order`, `static_native_locals`,
  `class_static_properties`.
- SV 6.21 L6897-6898: explicitly automatic variables "have the lifetime of the
  call or block and are initialized on each entry to the call or block".
  SV 6.21 L6900-6901: variables of an automatic task, function or block
  "default to the lifetime of the call or block and are initialized on each
  entry to the call or block". The automatic halves of `static_native_locals`
  and `fork_capture_lifetime`.
- SV 6.21 L6904-6905: "Specific variables within an automatic task, function,
  or block can be explicitly declared as static. Such variables have a static
  lifetime." SV 6.21 L6942-6944: "an explicit static keyword shall be
  required when an initialization value is specified as part of a static
  variable's declaration to indicate the user's intent of executing that
  initialization only once at the beginning of simulation."
  `static_native_locals`, `init_order` (a static local reads the value its
  source had before time 0 although a procedure changes it before the first
  call).
- SV 6.21 L6908-6909: "The lifetime of a fork-join block (see 9.3.2) shall
  encompass the execution of all processes spawned by the block. The lifetime
  of a scope enclosing any fork-join block includes the lifetime of the
  fork-join block." `fork_capture_lifetime`: detached children read and write
  the call's automatic string, queue, handle and record after the call
  returned; they share the variables, so they see the parent's later writes.
- SV 6.20.6 L6884-6886: "In other words, the object acts like a variable that
  cannot be written. The arguments to the new method shall be constant
  expressions (see 11.2.1). The members of the object can be written (except
  for those members that are declared const)." `neg_const_*`,
  `const_reads_and_pointees`.
- SV 8.8 L9870-9871, L9887, L9893: "Sometimes only one version of a variable
  is required to be shared by all instances. These class properties are
  created using the keyword static." "Now, fileID shall be created and
  initialized once." "The static class properties can be used without
  creating an object of that type." `class_static_properties`.
- SV 11.13 L16778-16779: "Variables used in a let that are not formal
  arguments to the let are resolved according to the scoping rules from the
  scope in which the let is declared." `let_environments`.
- SV 13.4.4 L18993-18997: "Calling a function that tries to schedule an event
  that cannot become active until after that function returns shall be
  allowed provided that the thread calling the function is created by an
  initial procedure, always procedure, or fork block from one of those
  procedures and in a context in which a side effect is allowed.
  Implementations shall issue an error either at compile time or run time
  when these provisions have not been met." `neg_initializer_*`,
  `initializer_effects_from_process`.
- SV 3.12.1 L2662-2667: compliant tools provide both "All files on a given
  compilation command line make a single compilation unit (in which case the
  declarations within those files are accessible following normal visibility
  rules throughout the entire set of files)" and "Each file is a separate
  compilation unit (in which case the declarations in each compilation-unit
  scope are accessible only within its corresponding file)". llg selects them
  with `--compilation-units merged|separate`. `unit_*`.
- SV 26.2 L45340-45341, L45344-45345: "The package declaration creates a
  scope that contains declarations intended to be shared among one or more
  compilation units, modules, interfaces, or programs." "It is also possible
  to populate packages with parameters, variables, and nets." SV 26.6
  L45812-45813, L45815: "The special wildcard export form, export *::*; ,
  exports all imported declarations from all packages from which imports
  occur." "An export of the form package_name::name makes the given
  declaration available." `package_state`.
- SV 18.14.1 L31148-31151, L31163-31166: "Each module instance, interface
  instance, program instance, and package has an initialization RNG. Each
  initialization RNG is seeded with the default seed." "When a
  class object is created by a static declaration initializer, there is no
  active thread; thus, the RNG of the created object is seeded with the next
  random value of the initialization RNG of the module instance, interface
  instance, program instance, or package in which the declaration occurred."
  `instance_object_streams`.
- V 6.2.1 L5664-5668: a Verilog-2001 variable declaration assignment "is a
  special case of procedural assignment", equivalent to an `initial`
  assignment; the SystemVerilog rule above replaces it only in the 2009
  edition, so llg keeps Verilog-2001 initializers as active-region processes.

## What each fixture checks

| Fixture | Expected result | Basis |
| --- | --- | --- |
| `static_native_locals` | Static string, queue, dynamic and associative array, class handle, record with string and queue members, semaphore and mailbox locals of an automatic function, a static function, a waiting task, a class method and a loop block keep their values across calls; their automatic counterparts restart from the initializer on every entry. | 6.21 L6897-6905, L6942-6944 |
| `init_order` | A package handle and the package values computed from it, a module handle read by later initializers, and static locals initialized from module state all hold their before-time-0 values, whichever procedure calls first and although one procedure changes the sources at time 0. | 6.8 L5286-5287, 6.21 L6942-6944 |
| `fork_capture_lifetime` | `join_none` children of two task activations print each activation's own automatic string, queue, handle and record 5 time units after the task returned, including the parent's writes after the fork. | 6.21 L6908-6909 |
| `class_static_properties` | Static string, handle, integral (from another property and from a call) and queue properties initialize once before time 0, are shared by all objects and the class scope, and a derived class's static property reads its base's. | 8.8, 6.8 |
| `instance_object_streams` | Objects built by static initializers of two identical instances have the same random state (every initialization RNG is seeded with the default seed); two objects of one instance have different random states. | 18.14.1 L31148-31151, L31163-31166 (S28-D4, S28-D7). Some simulators take no seed value for an object of a class without random variables, so both objects of one instance get the same state; with a `rand` member they agree. llg follows L31161-31163 (`SystemVerilog-1800-2009.txt`): "Each class instance (object) has an independent RNG ... When an object is created using new, its RNG is seeded with the next random value from the thread that creates the object" |
| `package_state` | Package string, queue, associative array, handle and record, and a package function's static locals, are one copy for two importing instances; names re-exported through `export p::name` and `export *::*` reach the same storage. | 26.2 L45340-45345, 26.6 L45812-45815 |
| `unit_shared_b` (+ `unit_shared_a`) | Merged: the second file reads and the first file's module writes the same `$unit` string, queue and handle. Separate: the second file's references are undeclared. | 3.12.1 L2662-2667 |
| `unit_isolated_b` (+ `unit_shared_a`) | Separate: each file has its own `$unit` variables of the same names. Merged: the second declaration of a name rejects with both locations. | 3.12.1 L2662-2667 |
| `let_environments` | Free names and default actuals bind at the let's declaration (module, package, `$unit`, block, task), actuals bind at the use, formals shadow outer names, nested lets compose, typed formals convert. | 11.13 L16778-16779 |
| `initializer_effects_from_process` | A fork and a nonblocking assignment in a function are legal from a procedure and from an automatic local's initializer inside one; an immediate event trigger is legal in a static initializer. | 13.4.4 L18993-18997 |
| `neg_initializer_fork`, `neg_initializer_nba`, `neg_initializer_constructor` | A static initializer whose call (directly, through a second function, or through a constructor) reaches a fork or a nonblocking assignment stops with the initializer's and the statement's locations. | 13.4.4 L18996-18997 |
| `const_reads_and_pointees` | Read-only methods of const queues and strings work, and a const class handle's object members (and a user method named `delete`) stay writable. | 6.20.6 L6884-6886 |
| `neg_const_queue_method`, `neg_const_string_method`, `neg_const_property_method` | `push_back` on a const queue, `putc` on a const string and `delete` on a const associative-array property reached through an object stop with the call's location. | 6.20.6 L6884 (S22-D4) |
| `neg_const_handle_write`, `neg_static_reads_automatic`, `neg_nba_automatic_string` | Assigning a const handle, a static initializer reading an automatic formal, and a nonblocking assignment to an automatic string keep the frontend's located errors. | 6.20.6; 6.21; 10.4.2 |
