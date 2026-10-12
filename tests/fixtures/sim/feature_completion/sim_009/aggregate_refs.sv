// SIM-009-A03: a waited `ref` formal of unpacked array type follows the
// elements its condition reads, for a module array and a procedural-block
// automatic array actual; the task recurses with the same formal (SV 9.4.3,
// 13.5.2).
module tb;
  int g[4];

  task automatic seek(ref int a[4], input int n);
    wait (a[n] == n + 10);
    $display("seek %0d at %0d", n, $time);
    if (n < 2) seek(a, n + 1);
  endtask

  task automatic total(ref int a[4], input int sum);
    wait (a[0] + a[1] + a[2] + a[3] == sum);
    $display("total %0d at %0d", sum, $time);
  endtask

  initial begin
    automatic int loc[4];
    fork
      seek(g, 0);
      seek(loc, 1);
      total(g, 33);
    join_none
    #1 g[0] = 10;
    #1 g[3] = 5;
    #1 g[1] = 11;
    #1 g[2] = 12;
    #1 loc[1] = 11;
    #1 loc[3] = 5;
    #1 loc[2] = 12;
    #1 g[3] = 0;
    #1 $finish;
  end
endmodule
