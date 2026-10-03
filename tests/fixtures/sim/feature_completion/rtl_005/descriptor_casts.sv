// SV2009 6.24.3, 7.6, 11.4.5, 11.4.11 on descriptor-backed fixed arrays:
// casts reshape and clear X/Z before comparison or storage; conditionals
// merge immediate elements; no operand is flattened into one packed value.
module tb;
  typedef logic [16:0] big_t [0:65536];
  typedef bit [16:0] big_bits_t [0:65536];
  typedef logic [16:0] even_t [0:65535];
  typedef logic [33:0] pair_t [0:32767];
  typedef bit [16:0] even_bits_t [0:65535];
  typedef logic [1:0] mid_t [0:4999];
  typedef bit [1:0] mid_bits_t [0:4999];
  big_t a, a2;
  big_bits_t b;
  even_t ev;
  even_bits_t evb;
  pair_t pr;
  mid_t ma, ma2;
  mid_bits_t mb;
  logic c, r1, r2;
  int calls;

  function automatic big_t make(input logic [16:0] v);
    big_t t;
    calls = calls + 1;
    t[3] = v;
    return t;
  endfunction

  initial begin
    calls = 0;
    a[0] = 17'bx;
    a[5] = 17'h1;
    b[5] = 17'h1;
    $display("A %b %b %b", big_bits_t'(a) == b, big_bits_t'(a) === b, big_bits_t'(a) != b);
    a2 = a;
    r1 = a == make(17'h2);
    $display("B %b %b %b %0d", a2 == a, a2 === a, r1, calls);
    a2 = big_t'(big_bits_t'(a));
    $display("C %h %h %h %h", a2[0], a2[1], a2[5], a[0]);
    c = 1'bx;
    $display("D %b %b", (c ? a : a) === a, (c ? big_bits_t'(a) : b) === b);
    a2 = a;
    a2[5] = 17'h2;
    r1 = (c ? a : a2) === a;
    r2 = (c ? make(17'h4) : make(17'h4)) === make(17'h4);
    $display("E %b %b %0d", r1, r2, calls);
    c = 1'b1;
    r1 = (c ? make(17'h4) : make(17'h5)) == make(17'h4);
    $display("F %b %0d", r1, calls);
    pr[0] = {17'h1, 17'h2};
    pr[1] = {17'bz, 17'h3};
    ev = even_t'(pr);
    $display("G %h %h %h %h %h", ev[0], ev[1], ev[2], ev[3], ev[4]);
    $display("H %b %b", even_t'(pr) === ev, pair_t'(ev) === pr);
    evb = even_bits_t'(pr);
    $display("I %h %h %h %h", evb[0], evb[2], evb[3], evb[4]);
    ma[0] = 2'bx1;
    ma[5] = 2'h1;
    mb[0] = 2'b01;
    mb[5] = 2'h1;
    $display("J %b %b", mid_bits_t'(ma) == mb, mid_bits_t'(ma) === mb);
    ma2 = mid_t'(mid_bits_t'(ma));
    $display("K %b %b %b", ma2[0], ma2[1], ma2[5]);
    c = 1'bx;
    $display("L %b %b", (c ? mid_bits_t'(ma) : mb) === mb, (c ? ma : ma2) == ma);
    $finish(0);
  end
endmodule
