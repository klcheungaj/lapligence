// Nonblocking writes to elements, rows and the whole of multidimensional
// native arrays (SV 10.4.2): sources are read when the assignment runs and
// the targets change in the NBA region.
module tb;
  typedef struct { string s; int n; } rec_t;
  rec_t nb[2][2];
  string ns[2][3];
  int k = 1;
  initial begin
    #1;
    nb[1][0] <= '{s: "nb", n: 4};
    ns[0][1] <= "e";
    ns[k] <= '{"x", "y", "z"};
    #0 $display("1 [%s] [%s] [%s]", nb[1][0].s, ns[0][1], ns[1][2]);
    #1;
    ns <= '{'{"a", "b", "c"}, '{default: "d"}};
    nb[0] <= nb[1];
    $display("2 %s%s%s %s%s%s %s %0d", ns[0][0], ns[0][1], ns[0][2], ns[1][0], ns[1][1], ns[1][2],
             nb[1][0].s, nb[1][0].n);
    #1;
    $display("3 %s%s%s %s%s%s %s %0d", ns[0][0], ns[0][1], ns[0][2], ns[1][0], ns[1][1], ns[1][2],
             nb[0][0].s, nb[0][1].n);
    ns[k - 1] <= ns[k];
    ns[k][2] <= ns[0][0];
    #1;
    $display("4 %s%s%s %s%s%s", ns[0][0], ns[0][1], ns[0][2], ns[1][0], ns[1][1], ns[1][2]);
    $finish(0);
  end
endmodule
