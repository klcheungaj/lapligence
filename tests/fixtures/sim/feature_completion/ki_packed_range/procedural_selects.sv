// IEEE 1800-2009 7.4.1, 7.4.5 and 11.5.1: a part-select or indexed
// part-select of a packed array selects whole elements of the outermost
// dimension it is applied to; an element select keeps the element type.
module tb;
  typedef struct packed { logic [3:0] hi; logic [3:0] lo; } pair_t;
  typedef union packed { logic [7:0] b; pair_t p; } byte_u;
  typedef logic signed [3:0] s4_t;
  logic [3:0][7:0] w;
  logic [0:3][7:0] a;
  logic [4:1][3:0] n;
  logic [2:0][1:0][3:0] x;
  pair_t [3:0] ps;
  byte_u [1:0] us;
  s4_t [1:0] se;
  bit [3:0][7:0] tw;
  integer i, j;
  initial begin
    w = 32'h44332211;
    a = 32'h44332211;
    n = 16'h4321;
    x = 24'h654321;
    $display("const %h %h %h %h %h", w[3:2], w[1:0], w[2 +: 2], w[2 -: 2], w[0 +: 1]);
    $display("ascending %h %h %h %h", a[0:1], a[2:3], a[1 +: 2], a[2 -: 2]);
    $display("offset %h %h %h", n[3:2], n[4 -: 2], n[1 +: 3]);
    $display("nested %h %h %h %h", x[2:1], x[1][0], x[2][1:0], x[0][1][3:2]);
    i = 1;
    j = 2;
    $display("runtime %h %h %h %h %h", w[j +: 2], w[i -: 2], w[j], x[i][j - 1], x[j][i -: 2]);
    i = 3;
    $display("high %h %h", w[i +: 2], w[i + 1]);
    i = -1;
    $display("low %h %h", w[i +: 2], a[i +: 2]);
    i = 'x;
    $display("unknown %h %h %h", w[i -: 2], w[i], x[i][0]);
    se = 8'h8f;
    $display("signed %0d %0d %0d", se[1], se[0], se[1:0]);
    ps = 32'h44332211;
    us = 16'h1234;
    i = 0;
    $display("records %h %h %h %h %h", ps[2:1], ps[3], ps[i], us[1:1], us[0]);
    tw = 32'h44332211;
    j = 1;
    $display("two-state %h %h", tw[3:2], tw[j +: 2]);

    w[2:1] = 16'hbbaa;
    $display("write %h", w);
    w[3 -: 2] = 16'hddcc;
    $display("write %h", w);
    i = 0;
    w[i +: 2] = 16'hffee;
    $display("write %h", w);
    j = 1;
    w[j][5:2] = 4'h0;
    $display("write %h", w);
    i = 3;
    w[i +: 2] = 16'h1234;
    i = -1;
    w[i +: 2] = 16'h5678;
    i = 'x;
    w[i -: 2] = 16'h0;
    $display("clipped %h", w);
    a[1:2] = 16'h0102;
    i = 2;
    a[i -: 2] = 16'h0a0b;
    $display("ascending %h", a);
    x[1] = 8'hab;
    x[0][1] = 4'hf;
    i = 2;
    j = 0;
    x[i][j] = 4'h0;
    x[i][j +: 2] = 8'h9c;
    $display("nested %h", x);
    ps[1:0] = 16'hbbaa;
    i = 2;
    ps[i] = 8'h0f;
    $display("records %h", ps);

    w = 0;
    w[2:1] <= 16'hbeef;
    i = 3;
    w[i -: 1] <= 8'h5a;
    #1 $display("nba %h", w);
    $finish(0);
  end
endmodule
