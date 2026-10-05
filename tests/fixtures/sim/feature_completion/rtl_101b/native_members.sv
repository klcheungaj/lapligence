// RTL-101b: column-layout records with real, string and chandle members
// cross subroutine boundaries and compare member by member.
module tb;
  typedef struct {
    bit [7:0] a [0:65536];
    real x;
    string s;
    chandle h;
    bit [3:0] k;
  } rec_t;
  rec_t r, q, z;
  logic c;
  function automatic rec_t f(input rec_t v);
    rec_t l;
    l = v;
    l.x = l.x * 2.0;
    l.s = {l.s, "?"};
    l.a[5] = l.a[5] + 8'd1;
    return l;
  endfunction
  function static rec_t g(input rec_t v);
    v.k = v.k + 4'd1;
    return v;
  endfunction
  function automatic bit same(input rec_t u, input rec_t v);
    return u == v;
  endfunction
  task automatic t(input rec_t i, output rec_t o, inout rec_t io);
    o = i;
    o.s = "out";
    io.x = io.x + 1.0;
    io.s = {io.s, "+"};
  endtask
  initial begin
    r.x = 1.25;
    r.s = "ab";
    r.k = 4'd3;
    r.a[5] = 8'd7;
    q = f(r);
    $display("A %f %s %0d %0d", q.x, q.s, q.a[5], q.k);
    q = r;
    $display("B %0d", q == r);
    q.s = "zz";
    $display("C %0d %0d", q == r, q != r);
    q = g(r);
    $display("D %0d %s", q.k, q.s);
    z.x = 0.5;
    z.s = "io";
    t(r, q, z);
    $display("E %s %f %s %f", q.s, q.x, z.s, z.x);
    q = (r.k == 4'd3) ? r : z;
    $display("F %s", q.s);
    q = '{a: '{default: 8'd1}, x: 2.5, s: "pat", h: null, k: 4'd9};
    $display("G %s %f %0d %0d", q.s, q.x, q.a[100], q.k);
    z = r;
    z.s = "cd";
    c = 1'bx;
    q = c ? r : z;
    $display("H [%s] %f %0d", q.s, q.x, q.k);
    $display("I %0d %0d %0d", same(r, z), same(r, r), q.h == null);
    $finish(0);
  end
endmodule
