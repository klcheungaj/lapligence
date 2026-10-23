// Decision R05-D1: an ambiguous conditional on unpacked arrays and
// structures merges them leaf by leaf. An element that is itself an unpacked
// array or structure is combined the same way, so each packed, real or other
// scalar leaf that matches in both arms keeps its value and only a leaf that
// differs takes its default-uninitialized value.
//
// IEEE 1800-2009 11.4.11 (SystemVerilog-1800-2009.txt L15213-15216): "both
// first expression and second expression shall be evaluated, and their
// results shall be combined element by element. If the elements match, the
// element is returned. If they do not match, then the default-uninitialized
// value for that element's type shall be returned." The text does not say
// whether an element that is itself an aggregate is replaced as a whole;
// llg follows industry-standard behaviour and recurses to the leaves.
//
// Every differing leaf below differs in all of its bits, so the default (X
// for logic, 0 for int and real) is also what merging its bits would give.
typedef struct { logic [3:0] a; int i; } rec_t;
typedef struct { rec_t r; logic [3:0] t [0:1]; real x; } nest_t;
typedef logic [3:0] row_t [0:1];
typedef struct { logic [7:0] w [0:4096]; rec_t r; bit [7:0] k; } wide_t;

module tb;
  logic c;
  row_t m1 [0:2], m2 [0:2], m [0:2];
  rec_t ra [0:1], rb [0:1], rr [0:1];
  nest_t n1, n2, n;
  rec_t sa [0:4096], sb [0:4096], sr [0:4096];
  wide_t w1, w2, w;
  rec_t da [], db [], dr [];
  initial begin
    c = 1'bx;
    m1 = '{'{4'h1, 4'h2}, '{4'h3, 4'h4}, '{4'h5, 4'h6}};
    m2 = '{'{4'h1, 4'h2}, '{4'h3, 4'hb}, '{4'ha, 4'h6}};
    m = c ? m1 : m2;
    $display("rows %h %h %h %h %h %h", m[0][0], m[0][1], m[1][0], m[1][1], m[2][0], m[2][1]);
    ra[0] = '{4'h1, 1}; ra[1] = '{4'h2, 32'h0f0f0f0f};
    rb[0] = '{4'h1, 1}; rb[1] = '{4'h2, 32'hf0f0f0f0};
    rr = c ? ra : rb;
    $display("records %h %0d %h %0d", rr[0].a, rr[0].i, rr[1].a, rr[1].i);
    n1.r = '{4'h5, 32'h0f0f0f0f}; n1.t = '{4'h1, 4'h2}; n1.x = 1.5;
    n2.r = '{4'h5, 32'hf0f0f0f0}; n2.t = '{4'h1, 4'hd}; n2.x = 1.5;
    n = c ? n1 : n2;
    $display("nested %h %0d %h %h %0.1f", n.r.a, n.r.i, n.t[0], n.t[1], n.x);
    sa[7] = '{4'h1, 32'h0f0f0f0f}; sb[7] = '{4'h1, 32'hf0f0f0f0};
    sa[8] = '{4'h3, 8}; sb[8] = '{4'hc, 8};
    sr = c ? sa : sb;
    $display("large %h %0d %h %0d", sr[7].a, sr[7].i, sr[8].a, sr[8].i);
    w1.w[3] = 8'd5; w2.w[3] = 8'd5; w1.w[4] = 8'h0f; w2.w[4] = 8'hf0;
    w1.r = '{4'h1, 32'h0f0f0f0f}; w2.r = '{4'h1, 32'hf0f0f0f0};
    w1.k = 8'd7; w2.k = 8'd7;
    w = c ? w1 : w2;
    $display("wide %0d %h %h %0d %0d", w.w[3], w.w[4], w.r.a, w.r.i, w.k);
    da = new[2]; db = new[2];
    da[1] = '{4'h2, 32'h0f0f0f0f}; db[1] = '{4'h2, 32'hf0f0f0f0};
    dr = c ? da : db;
    $display("dynamic %0d %h %0d", dr.size(), dr[1].a, dr[1].i);
    $finish(0);
  end
endmodule
