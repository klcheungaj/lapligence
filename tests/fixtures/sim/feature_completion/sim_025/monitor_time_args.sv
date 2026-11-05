// IEEE 1800-2009 21.2.3 L36517-36518 (SystemVerilog-1800-2009.txt): "each time a
// variable or an expression in the argument list changes value—with the
// exception of the $time, $stime, or $realtime system functions—the entire
// argument list is displayed at the end of the time step". An argument that
// computes only from those functions prints but triggers nothing (T1-D7).
`timescale 1ns/1ns
module tb;
  int arr[4];
  int q[$];
  int v, w;
  function automatic int f(int a[2]);
    return a[0] + a[1];
  endfunction
  function automatic int g(int unused);
    return arr[1];
  endfunction
  initial begin
    arr[1] = 1; arr[2] = 2;
    $monitor("%0t a %0d", $time, f(arr[1:2]));
    #1 arr[0] = 5;
    #1 arr[1] = 3;
    #1 arr[3] = 9;
    #1 arr[1] = 3;
    #1 arr[2] = 2;
    #1 $monitor("%0t b %0d %0t %0d", $time, v, $realtime, $stime);
    #1 v = 1;
    #1 w = 5;
    #1 v = 1;
    #1 $monitor("%t c %0d", $time, v + (w * 0));
    #1 w = 6;
    #1 v = 2;
    #1 $monitor("%0d %0t d", g(0), $time);
    #1 arr[0] = 8;
    #1 arr[1] = 1;
    #1 $monitor("%0t e", $time);
    #5 $monitor("%0t f %0d", $time + 1, v);
    #1 v = 7;
    #1 v = 7;
    #1 $monitor("%0d g", $time / 100);
    #200 $monitor("%0d h", ($time > 300) ? v : 3);
    #200 $finish(0);
  end
endmodule
