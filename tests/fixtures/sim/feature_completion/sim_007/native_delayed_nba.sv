// Intra-assignment delays on nonblocking writes to fixed arrays of strings,
// handles and native records (SV 10.4.2, 9.4.5): the source, the target
// position and the delay are evaluated when the assignment runs; the write
// commits in the NBA region of the delayed time step.
module tb;
  typedef struct { string s; int n; } rec_t;
  class C;
    int v;
    function new(int x); v = x; endfunction
  endclass
  string ss[3];
  rec_t rr[2];
  C hh[2];
  string mm[2][2];
  int k = 1;
  int d = 2;
  string src = "a";
  initial begin
    ss[k] <= #5 src;
    rr[0] <= #3 '{s: "r", n: 2};
    hh[1] <= #1 C'(new(7));
    ss <= #4 '{"x", "y", "z"};
    mm[1][0] <= #d "m";
    mm[0] <= #6 '{"p", "q"};
    ss[1:2] <= #7 '{"s", "t"};
  end
  initial begin
    #1;
    src = "late";
    k = 0;
    $display("1 [%s] [%s] [%s] %0d", ss[1], rr[0].s, mm[1][0], hh[1] == null);
    #1 $display("2 %0d [%s]", hh[1].v, mm[1][0]);
    #1 $display("3 [%s] %0d [%s]", rr[0].s, rr[0].n, mm[1][0]);
    #1 $display("4 [%s] %0d", rr[0].s, rr[0].n);
    #1 $display("5 [%s%s%s]", ss[0], ss[1], ss[2]);
    #1 $display("6 [%s%s%s] [%s%s]", ss[0], ss[1], ss[2], mm[0][0], mm[0][1]);
    #2 $display("7 [%s%s%s] [%s%s]", ss[0], ss[1], ss[2], mm[0][0], mm[0][1]);
    $finish(0);
  end
endmodule
