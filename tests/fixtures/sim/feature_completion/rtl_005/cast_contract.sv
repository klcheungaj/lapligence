// SV2009 6.19.4, 6.24.1-6.24.3: one success/failure contract for dynamic
// casts and state/width/sign/bit-stream conversion for static casts.
module tb;
  typedef enum logic [2:0] { A0 = 3'd0, A2 = 3'd2, A5 = 3'd5 } e_t;
  typedef enum bit [1:0] { B0, B1 } eb_t;
  typedef struct packed { e_t e; logic [3:0] v; } ps_t;
  typedef struct { logic [3:0] a; bit [1:0] b; } rec_t;
  typedef struct { bit [3:0] a; bit [1:0] b; } brec_t;
  typedef logic [2:0] l3_t [0:1];
  typedef bit [2:0] b3_t [0:1];
  typedef logic [1:0] l2_t [0:2];
  typedef logic [5:0] v6_t;
  e_t arr [0:3];
  e_t e;
  eb_t eb;
  ps_t ps;
  rec_t r, r2;
  brec_t br;
  l3_t l3, l3b;
  b3_t b3;
  l2_t l2;
  v6_t v6;
  int ok, idx, calls, srcs;
  logic [1:0] two;
  logic [7:0] w;
  bit [7:0] bw;
  logic signed [3:0] s4;
  logic signed [7:0] s8;
  logic [11:0] w12;
  real rr;

  function automatic int next_index();
    calls = calls + 1;
    return idx;
  endfunction
  function automatic logic [31:0] source(input logic [31:0] value);
    srcs = srcs + 1;
    return value;
  endfunction

  initial begin
    calls = 0; srcs = 0;
    foreach (arr[k]) arr[k] = A2;
    idx = 1;
    ok = $cast(arr[next_index()], source(32'd5));
    $display("A %0d %0d %0d %0d %0d", ok, arr[1], arr[0], calls, srcs);
    ok = $cast(arr[next_index()], source(32'd4));
    $display("B %0d %0d %0d %0d", ok, arr[1], calls, srcs);
    ok = $cast(arr[next_index()], source(32'd13));
    $display("C %0d %0d %0d %0d", ok, arr[1], calls, srcs);
    idx = 7;
    ok = $cast(arr[next_index()], source(32'd0));
    $display("D %0d %0d %0d %0d %0d %0d %0d", ok, arr[0], arr[1], arr[2], arr[3], calls, srcs);
    two = 2'bx1;
    idx = 2;
    ok = $cast(arr[two], source(32'd0));
    $display("E %0d %0d %0d %0d %0d", ok, arr[0], arr[1], arr[2], arr[3]);
    ps = {A5, 4'h3};
    ok = $cast(ps.e, source(32'd1));
    $display("F %0d %h", ok, ps);
    ok = $cast(ps.e, source(32'd0));
    $display("G %0d %h", ok, ps);
    eb = B1;
    two = 2'b1x;
    ok = $cast(eb, two);
    $display("H %0d %0d", ok, eb);
    two = 2'b00;
    ok = $cast(eb, two);
    $display("I %0d %0d", ok, eb);
    e = A2;
    ok = $cast(e, -1);
    $display("J %0d %0d", ok, e);
    ok = $cast(e, e);
    $display("K %0d %0d", ok, e);
    arr[0] = A5;
    ok = $cast(arr[1], arr[0]);
    $display("L %0d %0d %0d", ok, arr[0], arr[1]);
    w = 8'bxz10_1100;
    ok = $cast(bw, w);
    $display("M %0d %b", ok, bw);
    s4 = -4'sd2;
    ok = $cast(w, s4);
    $display("N %0d %b", ok, w);
    ok = $cast(rr, s4);
    $display("O %0d %0.1f", ok, rr);
    rr = 2.5;
    ok = $cast(w, rr);
    $display("P %0d %0d", ok, w);
    s8 = -8'sd3;
    w12 = 12'(s8);
    $display("Q %h %h %h", w12, 12'(unsigned'(s8)), 12'h0 + signed'(4'hf));
    v6 = 6'b1x0z10;
    r2 = rec_t'(v6);
    l3 = l3_t'(v6);
    $display("R %b %b %b %b", r2.a, r2.b, l3[0], l3[1]);
    b3 = b3_t'(l3);
    br = brec_t'(l3);
    $display("S %b %b %b %b %b", b3[0], b3[1], br.a, br.b, b3 == b3_t'(v6));
    l3 = '{3'b1x0, 3'b0z1};
    b3 = '{3'b100, 3'b001};
    l3b = l3_t'(b3_t'(l3));
    l2 = l2_t'(l3);
    $display("T %b %b %b %b %b %b %b", b3_t'(l3) == b3, l3b[0], l3b[1], l2[0], l2[1], l2[2],
             l3_t'(b3_t'(l3)) === l3);
    r = '{4'h3, 2'd1};
    $display("U %b %b %b", rec_t'(6'b0011_x1) === r, rec_t'(6'b0x11_01) == r,
             rec_t'(l3_t'(rec_t'(6'b0011_x1))) == r);
    $finish(0);
  end
endmodule
