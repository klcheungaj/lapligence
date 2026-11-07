// A whole `real` column copy writes only the cells whose bits differ, so an
// observer of a cell sees each change once, -0.0 and a NaN included.
`ifndef N
`define N 16
`endif
typedef struct { real d [`N]; int t; } r_t;

module tb;
  r_t r, q;
  int reals = 0;

  always @(q.d[3]) reals += 1;

  initial begin
    r.d[2] = 1.5;
    r.d[3] = 2.5;
    q = r;
    #1 $display("copy %f %f %0d", q.d[2], q.d[3], reals);
    r.d[3] = 3.5;
    q = r;
    #1 $display("change %f %0d", q.d[3], reals);
    q = r;
    #1 $display("same %0d", reals);
    r.d[3] = -0.0;
    q = r;
    #1 r.d[3] = 0.0;
    q = r;
    #1 $display("zero %0d", reals);
    r.d[3] = $bitstoreal(64'h7ff8000000000000);
    q = r;
    #1 $display("nan %0d", reals);
    q = r;
    #1 $display("nan again %0d %0d", reals, q.d[3] != q.d[3]);
    r.d[`N - 1] = 4.5;
    q = r;
    $display("last %f", q.d[`N - 1]);
    $finish(0);
  end
endmodule
