// SV 10.6.2, 10.6, 6.4: a forced element of a static unpacked array keeps
// its forced value through every other write of the array: procedural writes
// of the element, whole-array assignments, nonblocking assignments and the
// `reverse` method (SV 7.12.2; `sort` is decision S23-D9). Its right-hand
// side is reevaluated while the force is in effect, also when it reads
// another element. A released element keeps its value until the next
// procedural write, unless a continuous assignment drives it, which is
// reestablished.
module tb;
  int mem[4];
  int src[4];
  int cm[4];
  real r[3];
  int m2[2][3];
  int a = 1;
  int c = 1;
  int i;
  assign cm[1] = c;
  initial begin
    foreach (mem[k]) mem[k] = k;
    force mem[1] = a + 2;
    #1 $display("w1 %0d", mem[1]);
    a = 10;
    #1 $display("w2 %0d", mem[1]);
    src = '{100, 101, 102, 103};
    mem = src;
    #1 $display("w3 %0d %0d %0d %0d", mem[0], mem[1], mem[2], mem[3]);
    #1 $display("w4 %0d %0d %0d %0d", mem[0], mem[1], mem[2], mem[3]);
    release mem[1];
    #1 $display("w5 %0d", mem[1]);
    a = 20;
    #1 $display("w6 %0d", mem[1]);
    force mem[2] = mem[3] + 1;
    mem[3] = 40;
    #1 $display("w7 %0d %0d", mem[2], mem[3]);
    mem.reverse();
    #1 $display("w8 %0d %0d %0d %0d", mem[0], mem[1], mem[2], mem[3]);
    release mem[2];
    force r[2] = 2.5;
    r[2] = 1.0;
    #1 $display("w10 %f", r[2]);
    release r[2];
    r[2] = 3.0;
    #1 $display("w11 %f", r[2]);
    i = 1;
    force m2[i][2] = 12;
    m2[1][2] = 0;
    m2[1][1] = 11;
    #1 $display("w12 %0d %0d", m2[1][2], m2[1][1]);
    release m2[1][2];
    force cm[1] = 5;
    #1 $display("w13 %0d", cm[1]);
    release cm[1];
    #1 $display("w14 %0d", cm[1]);
    c = 2;
    #1 $display("w15 %0d", cm[1]);
    $display("done");
    $finish;
  end
  initial begin
    #3 mem[1] <= 55;
    mem[3] <= 33;
  end
endmodule
