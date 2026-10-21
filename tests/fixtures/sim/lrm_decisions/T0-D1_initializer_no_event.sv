// Decision T0-D1: the declaration initializer of a static variable is not a
// value change. It is applied before any initial or always procedure starts,
// so every procedure reads the initialized value from its first statement and
// no event control, edge, `always @*`, assertion clock or continuous
// assignment observes a time-0 transition from the type's default. This holds
// for module, package and interface variables of every type, explicitly
// static task variables and static class properties.
//
// IEEE 1800-2009 6.8 (SystemVerilog-1800-2009.txt L5286-5289):
//   "Setting the initial value of a static variable as part of the variable
//   declaration (including static class members) shall occur before any
//   initial or always procedures are started (also see 6.21 and 10.5 on
//   variable initialization with static and automatic lifetimes).
//   NOTE-In IEEE Std 1364-2005, an initialization value specified as part of
//   the declaration was executed as if the assignment were made from an
//   initial procedure, after simulation has started."
// IEEE 1800-2009 9.4.2 (L11767-11768):
//   "The execution of a procedural statement can be synchronized with a value
//   change on a net or variable or the occurrence of a declared event."
//
// Every observer below is an `always` procedure or assertion that can only
// see a change after it has started; the flags are printed at time 1.
package p;
  logic pv = 1'b0;
endpackage

interface ifc;
  logic iv = 1'b1;
endinterface

class C;
  static logic s = 1'b0;
endclass

module tb;
  typedef enum logic [1:0] {A = 0, B = 1, C2 = 2} e_t;
  typedef struct packed { logic [3:0] a; logic b; } s_t;
  logic l = 1'b0;
  logic h = 1'b1;
  bit b = 1'b1;
  int i = 5;
  real re = 1.5;
  e_t e = B;
  s_t st = '{4'h3, 1'b1};
  logic [7:0] pa = 8'h5;
  logic [3:0] ua [2] = '{4'h1, 4'h2};
  ifc i0();
  bit seen [15];
  logic comb_l;

  task t;
    static logic ts = 1'b0;
    ts = ~ts;
  endtask

  always @(l) seen[0] <= 1'b1;
  always @(negedge l) seen[1] <= 1'b1;
  always @(posedge h) seen[2] <= 1'b1;
  always @(b) seen[3] <= 1'b1;
  always @(i) seen[4] <= 1'b1;
  always @(re) seen[5] <= 1'b1;
  always @(e) seen[6] <= 1'b1;
  always @(st) seen[7] <= 1'b1;
  always @(pa) seen[8] <= 1'b1;
  always @(ua[1]) seen[9] <= 1'b1;
  always @(p::pv) seen[10] <= 1'b1;
  always @(i0.iv) seen[11] <= 1'b1;
  always @(t.ts) seen[12] <= 1'b1;
  always @(C::s) seen[13] <= 1'b1;
  always @* comb_l = l;
  a1: assert property (@(negedge l) 1'b1) seen[14] = 1'b1;

  initial begin
    $display("initial reads l=%b h=%b b=%b i=%0d re=%0.1f e=%0d st=%h pa=%h ua1=%h",
             l, h, b, i, re, e, st, pa, ua[1]);
    #1 $write("observed=");
    foreach (seen[k]) $write("%0d", seen[k]);
    $display(" comb_l=%b", comb_l);
    $finish;
  end
endmodule
