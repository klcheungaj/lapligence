// SV 10.6.2, 6.4: an element of a member array of a record, and a member of
// a record element of such an array, is a singular variable, so it is a
// force and release target, also with run-time indices evaluated when the
// statement executes. Built with `-D N=3` (one storage cell per record leaf)
// and `-D N=16` (the member arrays as columns); both print the same.
`ifndef N
`define N 16
`endif
typedef struct { int f; int g; } us_t;
typedef struct { int a; int arr[`N]; us_t u[`N]; } rec_t;
module tb;
  rec_t mod;
  rec_t fa[3];
  int i = 2, j = 1;
  initial begin
    force mod.arr[2] = 5;
    mod.arr[2] = 6;
    mod.arr[1] = 1;
    #1 $display("q1 %0d %0d", mod.arr[2], mod.arr[1]);
    force fa[1].arr[i] = 7;
    i = 0;
    fa[1].arr[2] = 8;
    fa[1].arr[0] = 3;
    #1 $display("q2 %0d %0d", fa[1].arr[2], fa[1].arr[0]);
    force mod.u[j].g = 11;
    mod.u[1].g = 12;
    mod.u[1].f = 13;
    #1 $display("q3 %0d %0d", mod.u[1].g, mod.u[1].f);
    force fa[j].u[j].f = 21;
    fa[1].u[1] = '{f: 1, g: 2};
    #1 $display("q4 %0d %0d", fa[1].u[1].f, fa[1].u[1].g);
    force mod.arr[i] = 4;
    mod = fa[1];
    #1 $display("q5 %0d %0d %0d %0d", mod.arr[0], mod.arr[2], mod.u[1].f, mod.u[1].g);
    i = 99;
    force fa[1].arr[i] = 99;
    force mod.arr[i] = 99;
    #1 $display("q6 %0d %0d", fa[1].arr[0], mod.arr[0]);
    i = 0;
    release mod.arr[2];
    release fa[1].arr[2];
    release mod.u[1].g;
    release fa[1].u[1].f;
    release mod.arr[i];
    mod.arr[2] = 1;
    fa[1].arr[2] = 1;
    mod.u[1].g = 1;
    fa[1].u[1].f = 1;
    mod.arr[0] = 1;
    #1 $display("q7 %0d %0d %0d %0d %0d", mod.arr[2], fa[1].arr[2], mod.u[1].g, fa[1].u[1].f, mod.arr[0]);
    $display("done");
    $finish;
  end
endmodule
