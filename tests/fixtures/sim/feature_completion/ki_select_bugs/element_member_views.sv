// Members of packed-array elements as continuous targets, port actuals,
// net drivers, force targets, combinational reads and event controls
// (IEEE 1800-2009 7.2.1, 9.2.2.2.1, 9.4.2, 10.3, 10.6.2, 23.3.3).
module inv(input logic [3:0] a, output logic [3:0] y);
  assign y = ~a;
endmodule

module tb;
  typedef struct packed { logic [3:0] hi; logic [3:0] lo; } pair_t;
  typedef struct packed { pair_t [1:0] arr; logic [3:0] tag; } holder_t;
  typedef pair_t [3:0] quad_t;
  pair_t [3:0] src;
  pair_t [1:0] cv;
  pair_t [1:0] out;
  pair_t [3:0] pw;
  wire pair_t [1:0] wn;
  holder_t h;
  logic [3:0] y0, a, c, x;
  integer i, k;

  assign wn[1].hi = 4'h9;
  assign wn[1].lo = 4'h8;
  assign wn[0] = 8'h76;
  assign cv[k].lo = y0;
  inv u0(.a(src[i+1].lo), .y(y0));
  inv u1(.a(4'h3), .y(out[1].hi));
  inv u2(.a(src[0].hi), .y(out[0].lo));
  always_comb a = h.arr[i].hi;
  always_comb c = src[i+1].hi;
  always_comb pw[k].hi = x;

  // Armed after `i` becomes 1: the write of src[1].hi must not wake it.
  initial begin
    #2 @(src[i].lo);
    $display("event src[%0d].lo %h", i, src[i].lo);
  end

  initial begin
    i = 0;
    k = 0;
    x = 4'h5;
    src = quad_t'(32'h89abcdef);
    h = holder_t'(20'h12345);
    #1 $display("t1 y0 %h cv %h out %h a %h c %h pw %h wn %h", y0, cv, out, a, c, pw, wn);
    i = 1;
    k = 2;
    #1 $display("t2 y0 %h cv %h a %h c %h pw %h", y0, cv, a, c, pw);
    k = 1;
    x = 4'h6;
    #1 $display("t3 cv %h pw %h", cv, pw);
    force wn[1].lo = 4'hf;
    src[1].hi = 4'h0;
    #1 $display("t4 wn %h", wn);
    release wn[1].lo;
    src[1].lo = 4'h4;
    #1 $display("t5 wn %h y0 %h", wn, y0);
    $finish(0);
  end
endmodule
