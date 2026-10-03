// V2001 4.1.13; SV2009 11.4.11 Table 11-20: reached arms, effect counts and
// immediate element/member merges, independent of constant folding.
typedef struct { logic [3:0] a; bit [2:0] b; int i; } rec_t;
typedef struct { rec_t r; logic [1:0] t [0:1]; } nest_t;
typedef logic [3:0] row_t [0:1];
typedef row_t mat_t [0:2];
typedef struct { logic [3:0] a; logic [1:0] b; } net_rec_t;

module tb;
  localparam logic PX = 1'bx;
  localparam logic P1 = 1'b1;
  int ca, cb;
  logic [3:0] x, y, v;
  logic c;
  logic [3:0] wide_sel;
  rec_t r, r1, r2, rm, rp;
  rec_t ra [0:1];
  rec_t rb [0:1];
  nest_t n1, n2, n;
  mat_t m;
  net_rec_t nv1, nv2;
  wire net_rec_t nw;

  assign nw = c ? nv1 : nv2;
  always_comb rm = c ? r1 : r2;

  function automatic rec_t fa(input logic [3:0] a);
    rec_t t;
    ca = ca + 1;
    t.a = a; t.b = 3'd3; t.i = 7;
    return t;
  endfunction
  function automatic rec_t fb(input logic [3:0] a);
    rec_t t;
    cb = cb + 1;
    t.a = a; t.b = 3'd4; t.i = 7;
    return t;
  endfunction
  function automatic logic [3:0] fv(input logic [3:0] value);
    ca = ca + 1;
    return value;
  endfunction
  function automatic mat_t fm(input logic [3:0] p, input logic [3:0] q);
    mat_t t;
    cb = cb + 1;
    t[0] = '{p, p}; t[1] = '{p, q}; t[2] = '{q, q};
    return t;
  endfunction
  function automatic rec_t pick(input logic s, input rec_t u, input rec_t w);
    return s ? u : w;
  endfunction

  initial begin
    ca = 0; cb = 0; x = 4'h1; y = 4'h2;
    v = PX ? x++ : y++;
    $display("A %b %0d %0d", v, x, y);
    v = P1 ? x++ : y++;
    $display("B %b %0d %0d", v, x, y);
    v = PX ? fv(4'h3) : fv(4'h3);
    $display("C %b %0d", v, ca);
    v = P1 ? fv(4'h3) : fv(4'h5);
    $display("D %b %0d", v, ca);
    v = PX ? fv(4'bzzzz) : fv(4'bzzzz);
    $display("E %b %0d %b", v, ca, PX ? 4'bzz01 : 4'bzz01);
    wide_sel = 4'bx1x0;
    r = wide_sel ? fa(4'h9) : fb(4'h6);
    $display("F %b %b %0d %0d %0d", r.a, r.b, r.i, ca, cb);
    wide_sel = 4'b0x00;
    r = wide_sel ? fa(4'h9) : fb(4'h6);
    $display("G %b %b %0d %0d %0d", r.a, r.b, r.i, ca, cb);
    r = PX ? fa(4'h3) : fa(4'h3);
    $display("H %b %b %0d %0d", r.a, r.b, r.i, ca);
    c = 1'bz;
    m = c ? fm(4'h1, 4'h2) : fm(4'h1, 4'h3);
    $display("I %h %h %h %h %h %h %0d", m[0][0], m[0][1], m[1][0], m[1][1], m[2][0], m[2][1], cb);
    m = (c ? (PX ? fm(4'h1, 4'h2) : fm(4'h1, 4'h2)) : fm(4'h1, 4'h2));
    $display("J %h %h %h %0d", m[0][0], m[1][1], m[2][1], cb);
    n1.r = '{4'h5, 3'd1, 9}; n1.t = '{2'b01, 2'b10};
    n2.r = '{4'h5, 3'd1, 9}; n2.t = '{2'b01, 2'b11};
    n = c ? n1 : n2;
    $display("K %b %b %0d %b %b", n.r.a, n.r.b, n.r.i, n.t[0], n.t[1]);
    n2.r.i = 10;
    n = c ? n1 : n2;
    $display("L %b %b %0d %b %b", n.r.a, n.r.b, n.r.i, n.t[0], n.t[1]);
    r = pick(1'bx, fa(4'h1), fb(4'h1));
    $display("M %b %b %0d %0d %0d", r.a, r.b, r.i, ca, cb);
    ra[0] = '{4'h1, 3'd1, 1}; ra[1] = '{4'h2, 3'd2, 2};
    rb[0] = '{4'h1, 3'd1, 1}; rb[1] = '{4'h2, 3'd3, 2};
    rp = c ? ra[3] : ra[0];
    $display("N %b %b %0d", rp.a, rp.b, rp.i);
    rp = c ? ra[3] : rb[5];
    $display("O %b %b %0d", rp.a, rp.b, rp.i);
    nv1 = '{4'h3, 2'd1}; nv2 = '{4'h3, 2'd2};
    r1 = '{4'h3, 3'd1, 4}; r2 = '{4'h5, 3'd1, 4};
    c = 1'bx;
    #1 $display("P %b %b %b %b %0d", nw.a, nw.b, rm.a, rm.b, rm.i);
    c = 1'b0;
    #1 $display("Q %b %b %b %b", nw.a, nw.b, rm.a, rm.b);
    r <= PX ? fa(4'h9) : fb(4'ha);
    #1 $display("R %b %b %0d %0d %0d", r.a, r.b, r.i, ca, cb);
    $finish(0);
  end
endmodule
