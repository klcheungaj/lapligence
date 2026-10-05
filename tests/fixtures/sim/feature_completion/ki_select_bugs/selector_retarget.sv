// A runtime-selected member target waits on its selector: a variable output
// port actual is an implied continuous assignment (IEEE 1800-2009 23.3.3.2,
// 10.3.2) and always_comb reads every selector it evaluates (9.2.2.2.1).
// A selector change re-evaluates and writes the newly selected element; the
// element selected before keeps its value, as for `assign a[i] = x`.
module pass(input logic [3:0] x, output logic [3:0] a);
  assign a = x;
endmodule

module tb;
  typedef struct packed { logic [3:0] hi; logic [3:0] lo; } pair_t;
  typedef struct packed { pair_t p; logic [1:0] t; } outer_t;
  pair_t s [2];
  pair_t c [2];
  outer_t m [2][2];
  pair_t [1:0] ps;
  logic [3:0] x, y;
  integer i, j, k;

  pass u0(.x(x), .a(s[i].lo));
  pass u1(.x(x), .a(ps[k].hi));
  always_comb c[i].lo = y;
  always_comb m[j][k].p.hi = y;

  initial begin
    i = 0;
    j = 0;
    k = 0;
    x = 4'h5;
    y = 4'h7;
    #1 $display("t1 s %h %h ps %h c %h %h m %h %h %h %h", s[0], s[1], ps, c[0], c[1],
                m[0][0], m[0][1], m[1][0], m[1][1]);
    i = 1;
    k = 1;
    #1 $display("t2 s %h %h ps %h c %h %h m %h %h %h %h", s[0], s[1], ps, c[0], c[1],
                m[0][0], m[0][1], m[1][0], m[1][1]);
    j = 1;
    x = 4'h6;
    #1 $display("t3 s %h %h ps %h c %h %h m %h %h %h %h", s[0], s[1], ps, c[0], c[1],
                m[0][0], m[0][1], m[1][0], m[1][1]);
    i = 2;
    y = 4'h3;
    #1 $display("t4 s %h %h c %h %h m %h", s[0], s[1], c[0], c[1], m[1][1]);
    $finish(0);
  end
endmodule
