// Decision S07-D15 (the run stops, so the expected output ends at the last
// line printed before the call, then the final procedure's line): with index
// checks on, a `ref` actual that selects no element ends the run at the call,
// even when the call sits two subroutine calls below the process. Neither the
// subroutine holding the call nor its callers run on, and no other process
// starts. With index checks off the callee binds the default and every line
// prints.
//
// IEEE 1800-2009 13.5.2 (SystemVerilog-1800-2009.txt L19138-19141): "When the
//   argument is passed by reference, both the caller and the subroutine share
//   the same representation of the argument; therefore, any changes made to
//   the argument, within either the caller or the subroutine, shall be
//   visible to each other."
// IEEE 1800-2009 7.4.6 (L8234-8236): "If an index expression is out of the
//   address bounds or if any bit in the address is X or Z, then the index
//   shall be invalid. The result of reading from an array with an invalid
//   index shall return the default uninitialized value for the array element
//   type."
// The text gives an element that does not exist no storage to share; llg and
// industry-standard behaviour end the run at the call.
module tb;
  int q[$];
  int k = 5;
  task automatic tr(ref int r);
    $display("tr %0d", r);
  endtask
  task automatic body();
    $display("body before");
    tr(q[k]);
    $display("body after");
  endtask
  task automatic outer();
    body();
    $display("outer after");
  endtask
  initial begin
    q.push_back(1);
    q.push_back(2);
    outer();
    $display("initial after");
    #1 $display("later");
    $finish;
  end
  initial #3 $display("other process");
  final $display("final");
endmodule
