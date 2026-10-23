// Whole unpacked-array values read and written as runs of dense cells
// (see readme.md for the expected values and the clauses they rest on).
typedef struct { logic [3:0] a; int i; } rec_t;
typedef rec_t recs_t [0:7];
typedef logic [7:0] bytes_t [0:7];

module tb;
  int A [10:1];
  int B [0:9];
  recs_t ra, rb, rc, rn, rt;
  logic [7:0] m [0:3][0:7];
  logic [7:0] m2 [0:1][0:7];
  bit [7:0] ba [0:7], bb [0:7];
  logic [7:0] seven [0:6], seven2 [0:6];
  logic [7:0] eight [0:7], eight2 [0:7];
  bytes_t src, w, v;
  bytes_t filled = '{default: 8'h5a};
  logic c;

  assign w = src;

  task automatic recs_out(input recs_t x, output recs_t y);
    y = x;
    y[0].i = -1;
  endtask

  task automatic bump(inout bytes_t b);
    foreach (b[k]) b[k] = b[k] + 8'd1;
  endtask

  function automatic bytes_t reversed(input bytes_t b);
    foreach (b[k]) reversed[k] = b[7 - k];
  endfunction

  initial begin
    foreach (B[k]) B[k] = k * 3;
    A = B;
    $display("A %0d %0d %0d", A[10], A[5], A[1]);
    B[1:8] = B[0:7];
    $display("shift %0d %0d %0d %0d", B[0], B[1], B[8], B[9]);

    foreach (ra[k]) ra[k] = '{4'(k), k * 10};
    rb = ra;
    rb[5].i = 1;
    c = 1'bx;
    rc = c ? ra : rb;
    $display("cond %h %0d %h %0d", rc[5].a, rc[5].i, rc[4].a, rc[4].i);
    c = 1'b0;
    rc = c ? ra : rb;
    $display("pick %0d %0d", rc[5].i, rc[6].i);

    ra[3].a = 4'bx01z;
    rb = ra;
    $display("rb %h %0d %b %0d %h %0d", rb[0].a, rb[0].i, rb[3].a, rb[3].i, rb[7].a, rb[7].i);
    $display("eq %b %b %b %b", ra == rb, ra === rb, ra != rb, ra !== rb);
    rb[5].i = 1;
    $display("ne %b %b", ra == rb, ra === rb);

    recs_out(ra, rt);
    $display("out %0d %0d %0d", rt[0].i, rt[1].i, rt[7].i);

    foreach (m[i, j]) m[i][j] = 8'(i * 16 + j);
    m2 = m[1:2];
    $display("m2 %h %h %h", m2[0][0], m2[0][4], m2[1][4]);
    m[0] = m[3];
    $display("m0 %h %h %h", m[0][0], m[0][4], m[1][0]);

    foreach (src[k]) src[k] = 8'(k + 1);
    src[1:6] = src[0:5];
    $display("overlap %p", src);
    #1 $display("cont %0d %0d %0d", w[0], w[1], w[7]);

    v = src;
    bump(v);
    $display("inout %p", v);
    v = reversed(v);
    $display("func %p", v);
    $display("fill %h %h", filled[0], filled[7]);
    v = '{default: 8'h33};
    v[2] = 8'h44;
    $display("default %p", v);
    rt = '{rec_t: '{4'hc, 7}};
    $display("records %h %0d %h %0d", rt[0].a, rt[0].i, rt[7].a, rt[7].i);

    foreach (ba[k]) ba[k] = 8'(100 + k);
    bb = ba;
    $display("bits %p %b", bb, ba == bb);

    foreach (seven[k]) seven[k] = 8'(k + 1);
    seven2 = seven;
    foreach (eight[k]) eight[k] = 8'(k + 8);
    eight2 = eight;
    $display("short %p %p", seven2, eight2);
    #20 $finish(0);
  end

  initial begin
    #10 rn <= ra;
    $display("nba-before %0d", rn[6].i);
    #1 $display("nba-after %0d %b", rn[6].i, rn[3].a);
  end
endmodule
