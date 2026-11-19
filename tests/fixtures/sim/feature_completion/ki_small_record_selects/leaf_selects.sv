// Selections of records of integral members held as one signal per leaf
// (member arrays below 16 cells): constant and run-time element, member and
// part selects, NBAs, `ref` actuals, whole copies and waits.
typedef struct packed {
  logic [3:0] hi, lo;
} pair_t;

typedef struct {
  pair_t pp[2];
  int z;
} inner_t;

typedef struct {
  logic [7:0] t;
  int a[3];
  pair_t p;
  logic [7:0] m[2];
  inner_t n;
} r_t;

module tb;
  r_t r, q;
  int k, v, j;

  task automatic bump(ref int x);
    x = x + 100;
  endtask

  initial begin
    j = 1;
    r.t = 8'h5a;
    r.a[0] = 1;
    r.a[1] = 11;
    r.a[2] = 21;
    r.p = 8'h3c;
    r.m[0] = 8'h01;
    r.m[1] = 8'h02;
    r.n.pp[0] = 8'h12;
    r.n.pp[1] = 8'h34;
    r.a[1] = 7;
    r.t[3:0] = 4'hf;
    r.p.hi = 4'h9;
    r.m[1][7] = 1'b1;
    r.n.pp[1].lo = 4'h7;
    r.n.z = -3;
    $display("c %h %0d %0d %0d %h %h %h", r.t, r.a[0], r.a[1], r.a[2], r.p, r.m[0], r.m[1]);
    $display("r %h %h %b %h", r.t[7:4], r.p.lo, r.m[1][7], r.m[1][3:0]);
    $display("m %h %h %0d %h", r.n.pp[1], r.n.pp[0], r.n.z, r.n.pp[1].hi);
    k = 2;
    r.a[k] = r.a[k] + 5;
    k = 0;
    r.a[k] += 1;
    k = 1;
    v = r.a[k];
    $display("k %0d %0d %0d %0d", r.a[0], r.a[1], r.a[2], v);
    k = 0;
    r.n.pp[k].hi = 4'hf;
    r.m[k] = r.m[k] + 8'h10;
    $display("p %h %h %h", r.n.pp[0], r.m[0], r.m[k][7:4]);
    #1 bump(r.a[0]);
    k = 2;
    bump(r.a[k]);
    $display("f %0d %0d", r.a[0], r.a[2]);
    q = r;
    $display("w %0d %0d %0d", q.a[0], q.a[2], q == r);
    q.a[1] = 0;
    $display("e %0d", q == r);
    #1 k = 1;
    r.a[k] = 55;
    #1 r.t[0] = 1'b0;
    #1 $display("z %0d %h", r.a[1], r.t);
    #2 $display("n %0d %0d %0d %h", r.a[0], r.a[1], r.a[2], r.p);
    $finish(0);
  end

  initial begin
    #5 r.a[2] <= 42;
    r.a[j] <= 43;
    r.p.lo <= 4'h1;
  end

  initial begin
    #1;
    @(r.a[1]);
    $display("a1 %0d %0d", r.a[1], $time);
    @(r.t);
    $display("r %h %0d", r.t, $time);
  end
endmodule
