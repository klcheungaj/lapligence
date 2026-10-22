# Port expressions

`tests/sim_port_expressions.rs` runs these fixtures in both optimizer modes.
A port in a module, interface or program header may name part of the inner
signals instead of one whole signal: a bit-select, a constant part-select
(also indexed, `+:`/`-:`), or a concatenation of these. Each part connects to
its bits of the actual in its own direction; an inout part joins the actual's
bits bidirectionally.

| Fixture | Covers |
| --- | --- |
| `concat_output.sv` | output concatenations of variables and nets, ordered and named, to an outside net and variable |
| `concat_input.sv` | input concatenations of nets and variables; the actual splits MSB part first |
| `concat_inout.sv` | an inout concatenation resolving with an outside driver |
| `selects.sv` | implicit and explicit bit- and part-selects of output variables, output nets and input nets, a partly driven outside net, a parameterized select |
| `inout_selects.sv` | inout part- and bit-selects, a concatenation mixing them, a `wand` inner net |
| `indexed.sv` | `+:` and `-:` port references (decision PX-D2) |
| `mixed_direction.sv` | one port mixing an input and an output (23.2.2.1 Example 7, decision PX-D1) |
| `ansi_explicit.sv` | explicitly named ANSI ports with selects and a concatenation (23.2.2.2, decision PX-D3) |
| `interface_program.sv` | port expressions in interface and program headers |
| `nonansi.v` | the Verilog-2001 forms, run with `--edition v2001` |
| `outside_forms.sv` | every outside lvalue form on whole ports, and the 23.2.2.1 examples that tie two ports to one inner net |
| `instance_arrays.sv` | generate-loop instances and arrays of instances whose actual is sliced per instance first (23.3.3.5) |
| `widths.sv` | output port expressions with a narrower and a wider actual (decision PX-D4 covers both directions) |
| `neg_*.sv` | located errors: a non-constant select, a non-lvalue output actual, a by-name connection of an unnamed port, an inout concatenation containing a variable, an inout port expression on a variable, an assignment to an input variable, an inout width difference |

When the actual and a port expression differ in width, only the low bits both
sides have are connected; a whole port widens or truncates instead (decision
PX-D4). An inout connection with a width difference is an error. A port
reference select outside the declared range is a frontend warning; its
out-of-range bits read x and ignore writes (11.5.1). Some simulators reject
such a select; llg keeps it, because 23.2.2.1 admits any part-select of a
vector declared within the module and 11.5.1 gives an out-of-range
part-select a meaning instead of making it illegal.

## Clauses

IEEE 1800-2009 23.2.2.1 (`SystemVerilog-1800-2009.txt` L40422-40434):

    port_expression ::=
         port_reference
       | { port_reference { , port_reference } }
    port_reference ::=
         port_identifier constant_select
    ...
    The port reference for each port in the list_of_ports in the module header can be one of the following:
       -      A simple identifier or escaped identifier
       -      A bit-select of a vector declared within the module
       -      A part-select of a vector declared within the module
       -      A concatenation of any of the above

IEEE 1800-2009 11.5.1 (L15784-15788):

    A part-select that addresses a range of bits that are completely out of the address bounds of the vector,
    packed array, packed structure, parameter or concatenation, or a part-select that is x or z shall yield the value
    x when read and shall have no effect on the data stored when written. Part-selects that are partially out of
    range shall, when read, return x for the bits that are out of range and shall, when written, only affect the bits
    that are in range.

IEEE 1800-2009 23.2.2.1 (L40589-40593):

    Example 7-Explicitly named port with mix of input and output directions (non-ANSI style module header)
          module mixed_direction (.p({a, e}));
          input a;              // p contains both input and output directions.
          output e;

IEEE 1800-2009 23.2.2.2 (L40678-40680, L40711-40713):

    ANSI style port declarations can be explicitly named, allowing elements of arrays and structures,
    concatenations of elements, and assignment pattern expressions of elements declared in a module, interface,
    or program to be specified on the port list.
    ...
    The self-determined type of the port expression becomes the type for the port. The port expression shall not
    be considered an assignment-like context. The port expression shall resolve to a legal expression for type of
    module port (see 23.3.3).

IEEE 1800-2009 23.3.3 (L41452-41455):

    Each port connection shall be a continuous assignment of source to sink, where one connected item shall be
    a signal source and the other shall be a signal sink. The assignment shall be a continuous assignment from
    source to sink for input or output ports. The assignment is a non-strength-reducing transistor connection for
    inout ports.

IEEE 1800-2009 23.3.3.3 (L41500-41503):

    -      An output can be connected to a net or variable (or a concatenation of nets or variables) of a
           compatible data type.
    -      An inout can be connected to a net (or a concatenation of nets) of a compatible data type or left
           unconnected, but cannot be connected to a variable.

IEEE 1364-2001 12.3.2 (`Verilog-1364-2001.txt` L12344-12350) lists the same
four port reference forms; 12.3.9.2 (L12705-12707) makes each port connection a
continuous assignment of source to sink.
