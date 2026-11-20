// The triggered method of a sequence declared in a clocking block: a located
// limit (docs/known_issues.md, UVM example differences).
module tb;
  bit clk;
  clocking mck @(posedge clk);
    sequence at_posedge;
      1;
    endsequence
  endclocking
  always #5 clk = ~clk;
  initial begin
    #7;
    if (!mck.at_posedge.triggered) @(mck);
    $display("%0t", $time);
    $finish(0);
  end
endmodule
