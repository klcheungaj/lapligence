// RTL-101: record member arrays beyond the dense-cell threshold are columns.
module tb;
  typedef logic [7:0] row_t [65537];
  typedef struct { row_t a; row_t b; } pair_t;
  typedef struct { bit [7:0] a [0:65536]; logic [3:0] tag; bit [2:0] n; } rec_t;
  pair_t pr;
  row_t d2, d3;
  rec_t r, s, u;
  bit [7:0] arr [0:65536];
  logic [7:0] x;
  integer i;
  logic sel;
  always_comb x = r.a[i];
  initial begin
    foreach (d2[k]) pr.a[k] = k[7:0];
    pr.b = pr.a;
    '{d2, d3} = pr;
    $display("A %h %h %h", d2[3], d3[65536], d3[257]);
    i = 5;
    r.a[5] = 8'h5a;
    r.tag = 4'h3;
    r.a[65536] = 8'h11;
    #1 $display("B %h %h %h %h %b", x, r.a[i], r.tag, r.a[65536], r.n);
    r.a[5] = 8'h5b;
    #1 $display("C %h", x);
    r.tag[1] = 1'b0;
    $display("D %b %h", r.tag, r.a[i][3:0]);
    arr = r.a;
    $display("E %h %0d %0d", arr[5], r.a == arr, r.a != arr);
    s = r;
    $display("F %0d %h %0d", s == r, s.a[65536], s === r);
    s.a[9] = 8'h01;
    $display("G %0d %0d", s == r, s != r);
    sel = 1'b0;
    u = sel ? r : s;
    $display("H %h", u.a[9]);
    sel = 1'bx;
    u = sel ? r : s;
    $display("I %h %h %b %b", u.a[9], u.a[5], u.tag, u.n);
    u <= r;
    $display("J %h", u.a[5]);
    #1 $display("K %h %b", u.a[5], u.tag);
    r = '{a: '{default: 8'h7}, tag: 4'h9, n: 3'd5};
    $display("L %h %h %h %0d", r.a[0], r.a[65536], r.tag, r.n);
    r = '{default: 0};
    $display("M %h %h %0d", r.a[5], r.tag, r.n);
    $finish(0);
  end
endmodule
