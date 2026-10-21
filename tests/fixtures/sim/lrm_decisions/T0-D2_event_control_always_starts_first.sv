// Decision T0-D2: at time 0, an `always` procedure whose statement is an
// event control (`always @(e) s`, including `always @*` and `always_ff`)
// starts before every other process. It is already waiting when continuous
// assignments, port connections and `initial` procedures make their time-0
// changes, so it observes them. Every other process keeps its order: an
// `always` that begins with a block (`always begin @(e) ... end`) and an
// `initial` start after those changes and do not observe them. A static
// variable's declaration initializer is not a change at all (T0-D1).
//
// IEEE 1800-2009 9.2 (SystemVerilog-1800-2009.txt L11066-11072):
//   "The initial and always procedures are enabled at the beginning of a
//   simulation. [...] There shall be no implied order of execution between
//   initial and always procedures. The initial procedures need not be
//   scheduled and executed before the always procedures."
// IEEE 1800-2009 4.9.1 (L3509-3511):
//   "A continuous assignment process is also evaluated at time 0 in order to
//   propagate constant values. This includes implicit continuous assignments
//   inferred from port connections (see 4.9.6)."
// IEEE 1800-2009 6.7 (L5199-5200):
//   "The default initialization value for a net shall be the value z. Nets
//   with drivers shall assume the output value of their drivers."
//
// The text leaves the start order open; llg chooses the order above. The
// flags are printed at time 1.
module child(input logic a, output wire o);
  bit neg_a, any_a;
  always @(negedge a) neg_a <= 1'b1;
  always @(a) any_a <= 1'b1;
  assign o = 1'b0;
endmodule

module tb;
  logic pa = 1'b0;
  logic v;
  logic u;
  wire wc = 1'b1;
  wire [1:0] wv;
  wire po;
  logic comb;
  bit pos_wc, blk_wc, ini_wc, pos_v, blk_u, any_po, ff_v;
  assign wv = 2'b01;
  child c(.a(pa), .o(po));

  initial v = 1'b1;
  initial u = 1'b1;
  initial @(wc) ini_wc = 1'b1;
  always @(posedge wc) pos_wc <= 1'b1;
  always begin
    @(posedge wc) blk_wc <= 1'b1;
    #10;
  end
  always @(posedge v) pos_v <= 1'b1;
  always begin
    @(posedge u) blk_u <= 1'b1;
    #10;
  end
  always @(po) any_po <= 1'b1;
  always @* comb = wv[0];
  always_ff @(posedge v) ff_v <= 1'b1;

  initial begin
    #1 $display("const net: posedge=%0d always-block=%0d initial=%0d", pos_wc, blk_wc, ini_wc);
    $display("initial write: posedge=%0d always-block=%0d always_ff=%0d", pos_v, blk_u, ff_v);
    $display("input port: negedge=%0d change=%0d", c.neg_a, c.any_a);
    $display("output port: change=%0d", any_po);
    $display("always @*: comb=%b", comb);
    $finish;
  end
endmodule
