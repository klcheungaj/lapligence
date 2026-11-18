# Static initializer order across initializer kinds

`kinds.sv` declares, in each of three scopes (the package `pkg`, the class
`holder`'s static properties and the module `tb`), one static initializer of
each kind llg lowers on its own path, interleaved: a scalar with a call, an
unpacked array pattern, a class handle `new`, a string, a queue pattern, an
unpacked struct pattern, a dynamic array pattern, an associative array
pattern and a last scalar. Each initializer calls a subroutine that appends
the initializer's name to its scope's `log` string, which has no initializer
itself, and no initializer reads another, so only the base order decides.

Decision S22-D8 orders independent initializers by scope, keeping
declaration order within a scope. Within each scope the log must therefore list the names
in declaration order, whatever the kind:
`scalar1 pattern handle string queue record dynamic assoc scalar2` for the
package, the class and the module. The order between the scopes is S22-D8's
and is not printed. The last line checks the stored values: `scalar1 +
scalar2` = 2, the first element of each pattern is the subroutine's 1, and
`pkg::assoc["k"] + holder::assoc[7]` = 2. Exit status 0.

The expected stdout was checked against an independent industry-standard
simulator, which prints the same lines. Before the fix, llg ran the array,
queue, dynamic and associative array patterns of a scope apart from its
other initializers (for example `pattern scalar1 handle string record scalar2
queue dynamic assoc` in the module).

Clauses (`SystemVerilog-1800-2009.txt`):

- 6.8 L5286-5288: "Setting the initial value of a static variable as part of
  the variable declaration (including static class members) shall occur
  before any initial or always procedures are started (also see 6.21 and 10.5
  on variable initialization with static and automatic lifetimes)." The text
  does not order the initializers among themselves; S22-D8 does.
