// Decision A19-D39: the time queue, callback lists, the user systf list and
// an iterator's reference. See docs/lrm_decisions.md for the decision and
// the expected output.
//
// IEEE 1800-2009 37.76 (SystemVerilog-1800-2009.txt L61480-L61493): a time
// queue has a time (vpi_get_time); detail 1 (L61491): "The time queue
// objects shall be returned in increasing order of simulation time.";
// detail 2 (L61492): "vpi_iterate() shall return NULL if there is nothing
// left in the simulation time queue."; detail 3 (L61493): "The current time
// queue shall only be returned as part of the iteration if there are events
// that precede read only sync." 37.75 (L61456-L61475): callbacks are placed
// on prim terms, expressions, time queues and statements; detail 1
// (L61474): vpi_get_cb_info() describes a callback object; detail 2
// (L61475): "To get callback objects not related to the above objects, the
// second argument to vpi_iterate() shall be NULL." 37.38 detail 6
// (L60096-L60097): "All user-defined system tasks or functions shall be
// retrieved using vpi_iterate(), with vpiUserSystf as the type argument,
// and a NULL reference argument." 37.79 details 1 and 2 (L61647-L61649):
// vpi_handle(vpiUse, iterator) is the reference the iterator was created
// from, and NULL for a NULL reference.
//
// Text decides (another simulator does not implement time queue iteration
// and stops on vpiUse of an iterator made from NULL): a time queue per
// distinct pending time, the current one only while events of the current
// slot precede read-only synchronization; a time callback is listed under
// its time queue, not under NULL; the NULL list holds the live callbacks
// placed on no object, in registration order.
`timescale 1ns / 1ns
module tb;
  reg a = 0, b = 0;
  always @(a) b = a;
  initial begin
    #2 $d39(1);
    #4 b <= 1;
    $d39(2);
    #1 $finish;
  end
  initial #4 a = 1;
  initial #10 a = 0;
endmodule
