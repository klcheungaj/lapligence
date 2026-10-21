# Output variable port initializers

`tests/sim_output_port_init.rs` runs these fixtures in both optimizer modes.
An output variable port declaration, ANSI or non-ANSI, may carry an
initializer. It initializes the port's internal variable exactly like a
variable declaration initializer.

## Clauses

IEEE 1800-2009 A.1.3 (`SystemVerilog-1800-2009.txt` L40609-40612):

    ansi_port_declaration ::=
         [ net_port_header | interface_port_header ] port_identifier { unpacked_dimension }
             [ = constant_expression ]
       | [ variable_port_header ] port_identifier { variable_dimension } [ = constant_expression ]

IEEE 1800-2009 A.2.1.2 (L40475-40477, with `list_of_variable_port_identifiers`
at L67526):

    output_declaration ::=
         output net_port_type list_of_port_identifiers
       | output variable_port_type list_of_variable_port_identifiers
    list_of_variable_port_identifiers ::= port_identifier { variable_dimension } [ = constant_expression ]

IEEE 1800-2009 6.8 (L5286-5288):

    Setting the initial value of a static variable as part of the variable declaration (including static class
    members) shall occur before any initial or always procedures are started

IEEE 1364-2001 12.3.3 (`Verilog-1364-2001.txt` L12386-12387, with A.2.3 at
L46413):

    | output reg [ signed ] [ range ]
      list_of_variable_port_identifiers
    list_of_variable_port_identifiers ::= port_identifier [ = constant_expression ]

IEEE 1364-2001 6.2.1 (L5664-5666):

    The variable declaration assignment is a special case of procedural assignment as it assigns a value to a vari-
    able. It allows an initial value to be placed in a variable in the same statement that declares the variable. The
    assignment shall be to a constant expression.

## Fixtures

| Fixture | Expected output and why |
| --- | --- |
| `ansi.sv` | `tb t0 u2.o=1 u2.n=5 u2.q=21 u3.q=8`: the initializers are set before `tb`'s `initial` starts (6.8). `q = P + 1` uses the overridden `P = 20` in `u2` and the default `P = 7` in `u3`. `tb t1 po=1 pn=5 pq=21 pb=a pe=10 changed=1`: the port connections carry the values out. The explicit port `.p(a)` initializes `a`. `always @(po)` sees the port net change from x to 1 at time 0 (decision T0-D2). `tb t1 io=1 in=3 qo=0 qn=4` and `program po=0 pn=4`: interface and program output ports initialize in the same way. The program's last `initial` procedure ends the run at time 3 (24.3), so the `#4 $finish` never runs. |
| `ansi_types.sv` | `r=2.500000 e=C s=34 k=111111 str=hi i=-3 ua=1,2`: real, enum, packed struct, parameter-sized (`K = '1` for `W = 6`), string, integer and unpacked-array (`{ variable_dimension } [ = constant_expression ]`) output ports. |
| `nonansi.sv` | `po=1 pq=9 pr=6`: non-ANSI `output logic`, `output int` and `output var logic` declarations with initializers. |
| `nonansi.v` | `--edition v2001`. `child o=1 q=9` and `tb po=1 pq=9`: `output reg` declarations with initializers (12.3.3, 6.2.1). |
| `neg_inout.sv` | Rejected at 2:28. An `inout` port has no initializer form in A.2.1.2, and a port default value is for inputs only (23.2.2.4). |
| `neg_output_net.sv` | Rejected at 2:28. An initializer on an output net port is neither a port default value (inputs only) nor a variable initializer. |

IEEE 1800-2009 23.2.2.4 (L40857):

    Defaults can be specified only for input ports and only in ANSI style declarations.
