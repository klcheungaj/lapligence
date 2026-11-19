// Invalid run-time and constant indices into member arrays of a record of
// integral members held as one signal per leaf: reads give the element
// default and writes change nothing (run with --no-index-checks).
typedef struct {
  logic [7:0] t;
  int a[3];
  logic [7:0] m[2];
} r_t;

module tb;
  r_t r;
  int k, v;
  integer u;
  logic [7:0] w;

  initial begin
    r.t = 8'h5a;
    r.a[0] = 1;
    r.a[1] = 2;
    r.a[2] = 3;
    r.m[0] = 8'h44;
    r.m[1] = 8'h55;
    k = 3;
    v = 9;
    v = r.a[k];
    r.a[k] = 99;
    $display("o %0d %0d %0d %0d", v, r.a[0], r.a[1], r.a[2]);
    k = -1;
    w = r.m[k];
    r.m[k] = 8'h00;
    $display("n %h %h %h", w, r.m[0], r.m[1]);
    u = 'x;
    v = 9;
    v = r.a[u];
    r.a[u] = 77;
    w = r.m[u];
    r.m[u] += 1;
    $display("x %0d %h %0d %0d %0d %h %h", v, w, r.a[0], r.a[1], r.a[2], r.m[0], r.m[1]);
    $display("t %h", r.t);
    $finish(0);
  end
endmodule
