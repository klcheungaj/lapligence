// SIM-023 A01: constant member, element and indexed selects of vector nets.
module tb;
  typedef struct packed {
    logic [3:0] hi;
    logic [3:0] lo;
  } ps_t;
  typedef struct packed {
    ps_t inner;
    logic [1:0] tag;
  } outer_t;
  logic [7:0] d;
  logic [9:0] od;
  wire ps_t sn;
  wire outer_t on;
  wire [1:0][3:0] pa;
  wire [8:1] n8;
  wire [0:7] nr;
  assign sn = d;
  assign pa = d;
  assign n8 = d;
  assign nr = d;
  assign on = od;
  initial begin
    d = 8'h12; od = 10'h3ff;
    force sn.hi = 4'hf;
    force pa[0] = 4'h9;
    force pa[1][0] = 1'b0;
    force on.inner.lo = 4'h0;
    force on.tag = 2'b01;
    force n8[4:3] = 2'b11;
    force nr[2 +: 2] = 2'b11;
    force n8[8 -: 2] = 2'b10;
    #1 $display("1 sn=%h pa=%h on=%b n8=%b nr=%b", sn, pa, on, n8, nr);
    d = 8'h34; od = 10'h000;
    #1 $display("2 sn=%h pa=%h on=%b n8=%b nr=%b", sn, pa, on, n8, nr);
    release sn.hi; release pa[0]; release pa[1][0]; release on; release n8[8:7];
    release nr;
    #1 $display("3 sn=%h pa=%h on=%b n8=%b nr=%b", sn, pa, on, n8, nr);
    release n8;
    #1 $display("4 n8=%b", n8);
    $finish;
  end
endmodule
