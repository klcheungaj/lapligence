// SV 14.3: a clocking signal shall not designate a variable restricted to a
// procedural block.
module tb;
  logic clk;
  task automatic t;
    int y;
    y = 1;
  endtask
  clocking cb @(posedge clk);
    input z = t.y;
  endclocking
endmodule
