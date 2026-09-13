// IEEE 1800-2009 §10.6.2: an automatic variable cannot outlive its task
// activation as a procedural force target.
`timescale 1ns/1ps
module tb;
  reg source;

  task automatic bad_force;
    reg local_value;
    begin
      local_value = source;
      force local_value = source;
    end
  endtask

  initial begin
    source = 1'b1;
    bad_force();
    $finish(0);
  end
endmodule
