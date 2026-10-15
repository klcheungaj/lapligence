// SIM-046 assertion handles and coverage through VPI (IEEE 1800-2009 39.3,
// 40.5). The application vpi_coverage.c prints what it reads; derivations:
// readme.md.
module unit(input logic clk, input logic a, input logic b);
  p: assert property (@(posedge clk) a |-> b) else $display("%0t %m failed", $time);
  c: cover property (@(posedge clk) a);
endmodule

module tb;
  logic clk = 0, a = 0, b = 0;
  unit u1(clk, a, b);
  unit u2(clk, a, b);
  always #5 clk = ~clk;

  initial begin
    $vpi_cov(0);
    a = 1'b1;
    b = 1'b1;
    #20 b = 1'b0;
    #10;
    im: assert (a);
    d: assert #0 (b) else $display("d failed");
    #1 $vpi_cov(1);
    $finish;
  end
endmodule
