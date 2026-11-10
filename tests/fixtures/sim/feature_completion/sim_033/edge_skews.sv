`timescale 1ns/1ns
// Edge input skews name an edge of the clock signal (SV 14.3): `negedge x`
// under `@(posedge clk)` takes the #1step sample at each negedge of `clk`;
// `posedge y` under the same posedge event is the #1step sample.
module tb;
  bit clk = 0;
  always #5 clk = ~clk;
  int x = 0, y = 0, z = 0;
  clocking cb @(posedge clk);
    input negedge x;
    input posedge y;
    input #1step z;
  endclocking
  initial begin
    #7  begin x = 1; y = 1; z = 1; end
    #5  begin x = 2; y = 2; z = 2; end
    #10 begin x = 3; y = 3; z = 3; end
    #2  begin x = 4; y = 4; z = 4; end
    #16 $display("%0t end", $time);
    $finish;
  end
  always @(cb) $display("%0t cb.x %0d cb.y %0d cb.z %0d (x now %0d)", $time, cb.x, cb.y, cb.z, x);
endmodule
