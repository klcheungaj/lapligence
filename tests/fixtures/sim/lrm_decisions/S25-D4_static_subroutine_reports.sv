// Decision S25-D4: $strobe and $monitor are legal in static tasks and
// functions; only variables of automatic subroutines are barred.
//
// IEEE 1800-2009 13.3.2 (SystemVerilog-1800-2009.txt L18646-18652):
//   "Because variables declared in automatic tasks are deallocated at the
//   end of the task invocation, they shall not be used in certain constructs
//   that might refer to them after that point: ... They shall not be traced
//   with system tasks such as $monitor and $dumpvars."
// IEEE 1800-2009 21.2.2 (L36482-36484): the $strobe action happens "just
//   before simulation time is advanced".
//
// The static task and function below name module variables and their own
// static formal; nothing is deallocated, so the reports are legal and print
// the values of the end of their slot.
module tb;
  reg [3:0] a = 1;

  task static show(input [3:0] v);
    $strobe("task strobe a=%0d v=%0d", a, v);
  endtask

  function static void watch();
    $monitor("function monitor a=%0d", a);
  endfunction

  initial begin
    show(4'd9);
    a = 2;
    #1 watch();
    a = 3;
    #1 a = 4;
    #1 $finish;
  end
endmodule
