// SIM-032 A02: when every program initial has ended, simulation finishes
// although module processes are still live (IEEE 1800-2009 24.3, 9.2.3).
program p;
  initial begin
    #25 $display("p end t=%0d", $time);
  end
endprogram

module tb;
  logic clk = 1'b0;
  int n = 0;
  always #5 clk = ~clk;
  always @(posedge clk) n++;
  p p0();
  final $display("final n=%0d t=%0d", n, $time);
endmodule
