// SIM-025 A01: a monitor issued in a class method keeps its object when no
// variable names it: of the three objects only the replaced `e` is collected.
`timescale 1ns/1ns
module tb;
  int g = 3;
  class C;
    int x;
    function new(int v);
      x = v;
    endfunction
    function void watch();
      $monitor("%0t x=%0d g=%0d", $time, x, g);
    endfunction
  endclass
  C d, e;
  initial begin
    d = new(10);
    d.watch();
    #1 d = null;
    #1 e = new(99);
    #1 e = new(98);
    #1 g = 5;
    #1 $finish(0);
  end
endmodule
