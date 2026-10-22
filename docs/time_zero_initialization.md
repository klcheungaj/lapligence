# Time-zero initialization

What a SystemVerilog design sees at simulation time 0, and how to write code
that behaves the same on every simulator. The register entries behind each
rule are listed in the [LRM decision register](lrm_decisions.md#time-zero-initialization-and-start-order-refsim-time0).

## The rule: a declaration initializer is not a value change

A static variable's declaration initializer is applied **before any procedure
starts**. Procedures read the initialized value from their first statement, and
the initialization is not a value change: no event control (`@(v)`), edge
(`@(posedge v)`, `@(negedge v)`), `always @*` or assertion clock sees an event
at time 0 because of it.

IEEE 1800-2009 6.8 states it:

> Setting the initial value of a static variable as part of the variable
> declaration (including static class members) shall occur before any initial
> or always procedures are started (also see 6.21 and 10.5 on variable
> initialization with static and automatic lifetimes).
>
> NOTE—In IEEE Std 1364-2005, an initialization value specified as part of the
> declaration was executed as if the assignment were made from an initial
> procedure, after simulation has started.

An `always @(v)` can only wake on a change it is already waiting for, and 6.8
places the initialization before the procedure starts, so there is nothing to
see. This applies to module, interface and package variables of every type, to
explicitly static task and function variables and to static class properties.

Some simulators treat the initializer as a time-0 change (the IEEE 1364-2005
behaviour that the note describes). llg follows the 1800 text. A design that
depends on either behaviour is not portable.

Verilog-2001 designs (`--edition v2001`) are different: there a declaration
assignment is an initial-time assignment (IEEE 1364-2001 6.2.1), so it can be a
time-0 change.

## Example: `always @*` against `always_comb`

```systemverilog
module tb;
  logic l = 0;
  logic y_star, y_comb;
  always @* y_star = l;     // waits for a change of l
  always_comb y_comb = l;   // runs once at time 0
  initial begin
    #1 $display("t=1 y_star=%b y_comb=%b", y_star, y_comb);
    l = 1'b1;
    #1 $display("t=2 y_star=%b y_comb=%b", y_star, y_comb);
    $finish;
  end
endmodule
```

llg prints:

```
t=1 y_star=x y_comb=0
t=2 y_star=1 y_comb=1
```

`y_star` stays X until `l` changes, because the initializer of `l` is not a
change. `y_comb` is 0 from time 0, because `always_comb` evaluates once at time
zero. IEEE 1800-2009 9.2.2.2.2 lists this difference:

> always_comb automatically executes once at time zero, whereas always @* waits
> until a change occurs on a signal in the inferred sensitivity list.

## Example: edge controls

```systemverilog
module tb;
  logic clk = 0;
  always @(negedge clk) $display("negedge at %0t", $time);
  initial begin
    #5 clk = 1'b1;
    #5 clk = 1'b0;
    #1 $finish;
  end
endmodule
```

The `always` procedure fires once, when `clk` falls at time 10. The initializer
`clk = 0` is no X to 0 transition, so there is no negedge at time 0.

## Time-0 sampled values of nets

Sampled value functions (`$sampled`, `$past`, `$rose`, `$fell`, `$stable`,
`$changed`) and concurrent assertions read values sampled in the Preponed
region. At time 0 that region comes before any process runs, continuous
assignments included, so a net is sampled as the value it holds then: X on the
bits a driver writes and z on undriven bits. A static variable is sampled as
its declaration initializer (or its type's default), because the initializer
is applied before simulation starts. The rule is the same in every context:
assertions, covers, sequences, deferred assertions and procedural code.

```systemverilog
`timescale 1ns/1ns
module tb;
  logic a = 1'b1, b = 1'b1;
  logic clk = 1'b0;
  wire w = a & b;
  initial begin
    $display("t=0 sampled=%b past=%b", $sampled(w), $past(w, 1, , @(posedge clk)));
    #1 clk = 1'b1;
    #1 $display("t=2 sampled=%b past=%b", $sampled(w), $past(w, 1, , @(posedge clk)));
    $finish;
  end
endmodule
```

llg prints:

```
t=0 sampled=x past=x
t=2 sampled=1 past=1
```

At time 0, `$sampled(w)` and `$past(w)` (no clock tick yet) give X, not
`a & b`: the net declaration assignment has not run in the Preponed region of
time 0. From the tick at time 1 on, `w` is sampled as 1. IEEE 1800-2009 16.9.3
states the time-0 rule:

> The value of an expression sampled in the Preponed region corresponding to
> time 0 is the result of evaluating the expression using the initial values
> of the variables comprising the expression. The initial value of a static
> variable is the value assigned in its declaration, or, in the absence of such
> an assignment, it is the default (or uninitialized) value of the
> corresponding type (see 6.8, Table 6-7). The initial value of any other
> variable or signal is the default value of the corresponding type (see 6.8,
> Table 6-7).

Some simulators give the driver's value (here 1) for a net that no assertion
reads, while they give X when an assertion reads it. llg uses one rule for
every net and context. Do not rely on the time-0 sample of a net: start
checking after the first clock edge, or sample a variable with a known
initial value.

## Portable style

- Use `always_comb` (or `always_latch`) for combinational logic. It evaluates
  at time 0 on every simulator, whatever the initializers do.
- Do not rely on a time-0 edge from an initializer. Drive clocks and resets
  from an `initial` procedure, make the first edge you depend on happen after
  time 0 (for example `#5 clk = 1'b1;`), and give state an explicit reset
  instead of relying on its initial value being seen as a change.
