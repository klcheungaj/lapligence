// FIN-001 G3 (F09-04): inside fork-join_any and fork-join_none blocks a
// `ref` formal may be read only in the initializer of a fork block-item
// declaration, which the forking process evaluates before any branch starts
// (SV 9.3.2). Each branch then reads its own copy.
`timescale 1ns / 1ns
module tb;
  int b = 5;
  task automatic spawn(ref int r, input int n);
    for (int i = 0; i < n; i++) begin
      fork
        automatic int k = r;
        automatic int d = 2 * i + 1;
        #d $display("%0t k=%0d", $time, k);
      join_none
      r = r + 1;
    end
    fork
      automatic int first = r * 10;
      #2 $display("%0t first=%0d", $time, first);
    join_any
  endtask
  initial begin
    spawn(b, 3);
    $display("%0t b=%0d", $time, b);
    #4 $finish;
  end
endmodule
