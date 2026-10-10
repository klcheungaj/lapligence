// SIM-023 A01: force/release of resolved nets versus variables.
module src_m (output logic [3:0] o, input [3:0] i);
  assign o = i;
endmodule

module tb;
  typedef enum logic [1:0] {A, B, C} e_t;
  typedef struct packed {
    logic [3:0] hi;
    logic [3:0] lo;
  } ps_t;
  reg a, b;
  reg [3:0] d1, d2;
  wire w;
  wand wa;
  wor wo;
  tri t;
  wire [3:0] m;
  assign w = a;
  assign wa = a;
  assign wa = b;
  assign wo = a;
  assign wo = b;
  assign t = a;
  assign m = d1;
  assign m = d2;
  logic [7:0] v;
  real r;
  e_t e;
  ps_t s;
  int i;
  bit [3:0] b2;
  logic [7:0] src, cv;
  assign cv = src;
  logic [7:0] k;
  assign k = 8'h55;
  real rs, rv;
  assign rv = rs * 2.0;
  logic [3:0] pi, pv;
  src_m u (.o(pv), .i(pi));
  initial begin
    a = 1'b1; b = 1'b0; d1 = 4'b1100; d2 = 4'b1010;
    v = 8'h01; r = 1.5; e = A; s = 8'h12; i = 5; b2 = 4'h1;
    src = 8'h40; rs = 1.25; pi = 4'h1;
    #1 $display("1 w=%b wa=%b wo=%b t=%b m=%b", w, wa, wo, t, m);
    $display("1 cv=%h k=%h rv=%g pv=%h", cv, k, rv, pv);
    force w = 1'b0; force wa = 1'b1; force wo = 1'b0; force t = 1'bz; force m = 4'b0101;
    force v = 8'hx5; force r = 2.25; force e = C; force s = 8'h34; force i = -3;
    force b2 = 4'bx1z0;
    force cv = 8'haa; force k = 8'h00; force rv = 0.5; force pv = 4'hc;
    #1 $display("2 w=%b wa=%b wo=%b t=%b m=%b", w, wa, wo, t, m);
    $display("2 v=%h r=%g e=%s s=%h i=%0d b2=%b", v, r, e.name(), s, i, b2);
    $display("2 cv=%h k=%h rv=%g pv=%h", cv, k, rv, pv);
    a = 1'b0; b = 1'b1; d1 = 4'b0000; d2 = 4'b0000;
    v = 8'h03; r = 9.0; e = B; s = 8'h00; i = 0; b2 = 4'h0;
    src = 8'h41; rs = 2.0; pi = 4'h2;
    #1 $display("3 w=%b wa=%b wo=%b t=%b m=%b", w, wa, wo, t, m);
    $display("3 v=%h r=%g e=%s s=%h i=%0d b2=%b", v, r, e.name(), s, i, b2);
    $display("3 cv=%h k=%h rv=%g pv=%h", cv, k, rv, pv);
    release w; release wa; release wo; release t; release m;
    release v; release r; release e; release s; release i; release b2;
    release cv; release k; release rv; release pv;
    $display("4 w=%b wa=%b wo=%b t=%b m=%b", w, wa, wo, t, m);
    $display("4 v=%h r=%g e=%s s=%h i=%0d b2=%b", v, r, e.name(), s, i, b2);
    #1 $display("5 cv=%h k=%h rv=%g pv=%h", cv, k, rv, pv);
    v = 8'h03; r = 9.0; e = B; s = 8'h00; i = 0; b2 = 4'h0;
    src = 8'h42;
    #1 $display("6 v=%h r=%g e=%s s=%h i=%0d b2=%b cv=%h", v, r, e.name(), s, i, b2, cv);
    $finish;
  end
endmodule
