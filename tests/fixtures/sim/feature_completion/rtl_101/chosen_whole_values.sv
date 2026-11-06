// Whole copies and comparisons of chosen column records read their operands
// in place and write only the cells that differ: observers, unknown cells,
// cells of one side only and overlapping operands behave as with an image.
`timescale 1ns / 1ns

typedef struct {
  string name;
  int a[16];
  logic [7:0] b[16];
} rec_t;

typedef struct {
  logic [7:0] tag;
  logic [7:0] b[16];
} fix_t;

module tb;
  rec_t p, q, r;
  fix_t f, g;
  int k = 2;
  logic go = 0;

  always @(q.a[3]) $display("%0t q.a[3]=%0d", $time, q.a[3]);
  always @(q.b[4]) $display("%0t q.b[4]=%h", $time, q.b[4]);
  always @(g.b[5]) $display("%0t g.b[5]=%h", $time, g.b[5]);

  always @(posedge go) q <= p;

  task automatic swap_in(ref fix_t x, ref fix_t y);
    x = y;
    x.b[1] = 8'h11;
  endtask

  initial begin
    p.name = "p";
    p.a[k] = 0;
    p.a[k + 1] = 1;
    #1 q = p;
    $display("copy %0d %0d %s", q.a[2], q.a[3], q.name);
    #1 q = p;
    #1 p.a[3] = 5;
    q = p;
    p.a[3] = 6;
    $display("not an alias %0d %0d", q.a[3], p.a[3]);
    #1 q.a[7] = 9;
    q = p;
    $display("cell of one side %0d", q.a[7]);
    r = q;
    $display("eq %b %b", r == q, r === q);
    #1 p.b[k + 2] = 8'h5a;
    q = p;
    $display("eq unknown %b %b %b", p == q, p === q, p != q);
    r = q;
    r.b[6] = 8'h00;
    $display("neq unknown %b %b", r == q, r === q);
    r = q;
    r.a[k] = 1;
    $display("known mismatch %b %b", r == q, r === q);
    q.b[4] = 8'hxz;
    r = q;
    $display("copied x %h %b", r.b[4], r === q);
    q = q;
    #1 f.tag = 1;
    f.b[5] = 8'h77;
    g = f;
    #1 g.b[5] = 8'h01;
    #1 g = f;
    #1 swap_in(f, f);
    $display("self ref %h %h", f.b[1], f.b[5]);
    #1 swap_in(g, f);
    $display("two refs %h %h", g.b[1], g.b[5]);
    go = 1;
    #0 p.a[3] = 8;
    $display("before nba %0d", q.a[3]);
    #1 $display("after nba %0d %0d", q.a[3], p.a[3]);
    $finish(0);
  end
endmodule
