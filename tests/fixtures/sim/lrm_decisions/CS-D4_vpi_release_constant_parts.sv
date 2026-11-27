// Decision CS-D4: vpi_put_value with vpiReleaseFlag re-establishes every
// continuous assignment of the released variable, also one that assigns a
// constant to some of its bits and ran once: a constant part, bit, indexed
// part, packed structure member or concatenation operand of a variable, a
// constant element or element part of an array, a constant of a two-state
// variable (X and Z bits read as 0), a fill literal, a call of a function
// with constant arguments and a real constant. These hold the driven value
// before vpi_put_value returns; the bits that assignments of expressions
// drive follow when those run (the case reads everything one unit later).
//
// IEEE 1800-2009 38.34 (SystemVerilog-1800-2009.txt L64189-64190):
// "vpiReleaseFlag The object shall be released from a forced value (same as
// the SystemVerilog procedural release)."
// 10.6.2 L13391-13393: "Releasing a variable that is driven by a continuous
// assignment or currently has an active assign procedural continuous
// assignment shall reestablish that assignment and schedule a reevaluation
// in the continuous assignment's scheduling region."
// Some simulators keep the forced value of the real variable `r` after its
// release (they do so for a release statement too, CS-D3); llg
// re-establishes it as the clause requires: "released" shows r=2.500000.
`timescale 1ns / 1ns
module tb;
  typedef struct packed {
    logic [3:0] hi;
    logic [3:0] lo;
  } pair_t;
  function automatic logic [3:0] next(input int k);
    return k[3:0] + 4'd1;
  endfunction
  logic [7:0] in = 8'h31;
  logic [7:0] v, b, c, m, g, h, x, y;
  wire [7:0] n;
  pair_t s;
  logic [7:0] a[0:1];
  logic [7:0] e[1:0][0:2];
  logic [0:7] u;
  bit [7:0] t;
  real r;
  assign v[3:0] = 4'd5;
  assign v[7:4] = in[3:0];
  assign b[0] = 1'b1;
  assign b[7:1] = in[6:0];
  assign c[3:0] = 4'd5;
  assign c[7:4] = 4'd9;
  assign m[1:0] = 2'b10;
  assign m[5:4] = 2'b01;
  assign m[7:6] = in[1:0];
  assign m[3:2] = in[3:2];
  assign n[3:0] = 4'd5;
  assign n[7:4] = in[3:0];
  assign s.hi = 4'd5;
  assign s.lo = in[3:0];
  assign a[1] = 8'd9;
  assign a[0] = in;
  assign {g[3:0], h[1:0]} = 6'h15;
  assign g[7:4] = in[3:0];
  assign h[7:2] = in[5:0];
  assign u[0:3] = 4'h9;
  assign u[5] = 1'b1;
  assign u[4] = in[0];
  assign u[6:7] = in[2:1];
  assign e[1][2][3:0] = 4'h7;
  assign e[1][2][7:4] = in[3:0];
  assign t[3:0] = 4'b1x0z;
  assign t[7:4] = in[3:0];
  assign x[7-:2] = '1;
  assign x[5:0] = in[5:0];
  assign y[3:0] = next(2);
  assign y[7:4] = in[7:4];
  assign r = 2.5;

  task show(input string label);
    $display("%s: v=%h b=%h c=%h m=%h n=%h s=%h a0=%h a1=%h g=%h h=%h", label, v, b, c, m, n, s,
             a[0], a[1], g, h);
    $display("%s: u=%h e=%h t=%b x=%h y=%h r=%f", label, u, e[1][2], t, x, y, r);
  endtask

  initial begin
    #1 show("start");
    $d4c(1);
    #1 show("forced");
    $d4c(0);
    #1 show("released");
    in = 8'h42;
    #1 show("operand");
    $finish;
  end
endmodule
