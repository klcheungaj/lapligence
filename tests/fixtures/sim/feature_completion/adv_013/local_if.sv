// ADV-013 A02: `if` and `case` conditions that read a local variable
// assigned by the antecedent (16.13.5, 16.13.16, 16.10). Expected lines are
// derived in readme.md.
`timescale 1ns/1ns
module tb;
  logic clk = 0, a = 0, b = 0;
  logic [3:0] d = 0;
  always #5 clk = ~clk;
  property pr;
    logic [3:0] x;
    (a, x = d) |=> if (x == 4'd5) (nexttime !b) else (nexttime b);
  endproperty
  property pc;
    logic [3:0] x;
    (a, x = d) |=> case (x)
      4'd5: (nexttime !b);
      4'd6, 4'd7: (nexttime !b);
      default: (nexttime (d == 4'd15));
    endcase
  endproperty
  p1: assert property (@(posedge clk) pr) $display("p1 P %0t", $time); else $display("p1 F %0t", $time);
  p2: assert property (@(posedge clk) pc) $display("p2 P %0t", $time); else $display("p2 F %0t", $time);
  initial begin
    #12 a = 1'b1; d = 4'd5;
    #10 a = 1'b0;
    #10 b = 1'b1;
    #10 a = 1'b1; d = 4'd6; b = 1'b0;
    #10 a = 1'b0;
    #10 b = 1'b0;
    #20 $finish;
  end
endmodule
