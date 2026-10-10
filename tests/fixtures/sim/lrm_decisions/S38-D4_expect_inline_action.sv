// Decision S38-D4: the action block of an expect statement runs in the
// process that executed the expect, after it unblocks, so it can assign the
// automatic variables of a task (the LRM's own wait_for example; the awaited
// value is a static variable here because llg does not yet read automatic
// variables inside the expect property).
//
// IEEE 1800-2009 16.18 (SystemVerilog-1800-2009.txt L27951-27952): "The
// statement following the expect is scheduled to execute after processing the
// Observed region in which the property completes its evaluation."
// L27982-27985: "Because it is a blocking statement, the property can refer to
// automatic variables as well as static variables. ... The second argument,
// success, is used to return the result of the expect statement: 1 for
// success and 0 for failure."
`timescale 1ns / 1ns
module tb;
  logic clk = 1'b0;
  integer data = 0, target = 0;
  task automatic wait_for(output bit success);
    expect (@(posedge clk) ##[1:10] data == target) success = 1;
    else success = 0;
  endtask
  initial begin
    bit ok;
    target = 23;
    wait_for(ok);
    $display("%0t ok=%0d", $time, ok);
    target = 7;
    wait_for(ok);
    $display("%0t ok=%0d", $time, ok);
  end
  initial begin
    repeat (2) begin
      #5 clk = 1'b1;
      #5 clk = 1'b0;
    end
    data = 23;
    repeat (14) begin
      #5 clk = 1'b1;
      #5 clk = 1'b0;
    end
    $finish;
  end
endmodule
