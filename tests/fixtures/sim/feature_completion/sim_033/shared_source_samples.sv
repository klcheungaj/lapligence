`timescale 1ns/1ns
module tb;
  bit clk = 1'b0;
  bit clk2 = 0;
  int a = 0;
  // One source read through #1step, a positive skew and two #0 clockvars
  // in separate blocks, over several clocking events.
  clocking c1 @(posedge clk);
    input x = a;
  endclocking
  clocking c2 @(posedge clk);
    input #3 y = a;
    input #0 z = a;
  endclocking
  clocking c3 @(clk2);
    input #0 w = a;
  endclocking
  initial begin
    repeat (6) begin
      #2 a = a + 1;
      #3 clk = 1'b1; a = a + 10;
      clk2 = ~clk2;
      #1 a = a + 100;
      clk2 = ~clk2;
      #4 clk = 1'b0;
    end
    #1 $finish;
  end
  // Clockvars hold their samples until the next clocking event, so the
  // delayed read avoids racing the c3 process at the same time.
  always @(c2) #2 $display("%0t c2 x=%0d y=%0d z=%0d", $time, c1.x, c2.y, c2.z);
  always @(c3) $display("%0t c3 w=%0d", $time, c3.w);
endmodule
