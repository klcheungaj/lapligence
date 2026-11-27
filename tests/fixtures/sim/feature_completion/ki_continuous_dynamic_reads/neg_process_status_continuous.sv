// status() in a net declaration assignment: a method of the process object
// a class handle names, which 6.21 limits to procedural blocks.
module tb;
  process p;
  wire done = (p != null) && (p.status() == process::FINISHED);
  initial begin
    fork begin p = process::self(); #1; end join_none
    #2 $display("%0d", done);
    $finish(0);
  end
endmodule
