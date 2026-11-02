// Decision A19-D77: handles to the automatic variables of an ended frame.
// See docs/lrm_decisions.md for the decision and the expected output.
//
// IEEE 1800-2009 37.39 detail 1 (SystemVerilog-1800-2009.txt L60160):
// "Frames correspond to the set of automatic variables declared in a given
// task or function." 37.3.8 (L57568): a handle "remains valid only as long
// as the object exists". 38.36.1 (L64661-L64664): cbEndOfFrame "Triggers
// when a frame's associated task or function completes execution and
// indicates that the frame is about to end. When control is returned from
// this callback, any handles to this frame, its automatic variables, or
// their subelements should be considered invalid."
//
// Text decides (another simulator differs): an automatic variable exists
// while its frame does. Once the frame has ended, every use of a handle to
// one of its automatic variables fails with an error, also while a later
// activation of the same task runs; another simulator lets such a handle
// name the storage of a later activation.
module tb;
  int res;
  task automatic t(input int a);
    int loc = a * 10;
    $d77(1);
    res = loc;
  endtask
  initial begin
    t(1);
    $d77(2);
    t(2);
    $d77(3);
    $display("res=%0d", res);
    $finish;
  end
endmodule
