// Decision S23-D3: a constant member select of a packed-structure net, or a
// constant element select of a packed-array net, is a constant part-select of
// a vector net and can be forced and released on its own.
//
// IEEE 1800-2009 10.6.2 (SystemVerilog-1800-2009.txt L13373-13374):
//   "The left-hand side of the assignment can be a reference to a singular
//   variable, a net, a constant bit-select of a vector net, a constant
//   part-select of a vector net, or a concatenation of these."
// IEEE 1800-2009 7.2.1 (L7696-7700):
//   "A packed structure is a mechanism for subdividing a vector into
//   subfields, which can be conveniently accessed as members. ... when a
//   packed structure appears as a primary, it shall be treated as a single
//   vector."
// IEEE 1800-2009 11.5.1 (L15781-15782): "an indexed part-select is a
//   constant part-select if its base is a constant value as well as its
//   width."
//
// llg overlays only the selected bits; the other bits keep following the
// net's driver, and a release re-resolves only the selected bits.
module tb;
  typedef struct packed {
    logic [3:0] hi;
    logic [3:0] lo;
  } ps_t;
  logic [7:0] d;
  wire ps_t sn;
  wire [1:0][3:0] pa;
  assign sn = d;
  assign pa = d;
  initial begin
    d = 8'h12;
    force sn.hi = 4'hf;
    force pa[0] = 4'h9;
    force pa[1][2 +: 2] = 2'b11;
    #1 $display("forced sn=%h pa=%h", sn, pa);
    d = 8'h34;
    #1 $display("driver changed sn=%h pa=%h", sn, pa);
    release sn.hi;
    release pa[0];
    #1 $display("released sn=%h pa=%h", sn, pa);
    $finish;
  end
endmodule
