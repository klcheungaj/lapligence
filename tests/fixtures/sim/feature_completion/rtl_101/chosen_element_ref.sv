// A `ref` formal of a record with a string member and a member array of the
// column size, bound to an element of an array of records and of a queue: a
// store through the formal is a store to the element (SV 13.5.2).
`timescale 1ns/1ns
module tb;
  typedef struct { string s; int n; int a[16]; int m; } rec_t;
  rec_t ar[2];
  rec_t q[$];
  int seen = 0;
  task automatic inner(ref int y);
    y = 1;
  endtask
  task automatic w(ref rec_t x, input int v);
    #1 x.n = v;
    #1 x.s = "w";
    #1 x.n = v + 1;
    #1 x.a[1] = v;
    #1 x.n++;
    #1 inner(x.m);
  endtask
  always_comb seen = ar[1].n + 1;
  initial begin
    q.push_back(ar[0]);
    fork
      w(ar[1], 5);
      w(q[0], 7);
      begin wait (q[0].n == 7) $display("%0t q n", $time); end
      begin wait (ar[1].n == 5) $display("%0t ar n", $time); end
      begin wait (ar[1].s == "w") $display("%0t ar s", $time); end
      begin wait (ar[1].a[1] == 5) $display("%0t ar a", $time); end
      begin wait (ar[1].n == 7) $display("%0t ar inc", $time); end
      begin wait (ar[1].m == 1) $display("%0t ar inner", $time); end
    join
    #1 $display("seen %0d", seen);
    $finish;
  end
endmodule
