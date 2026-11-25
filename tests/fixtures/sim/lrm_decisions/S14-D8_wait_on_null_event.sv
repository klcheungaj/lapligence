// Decision S14-D8: waiting on a null event is a run-time error that ends the
// simulation at once, like an access through a null class handle: the waiting
// process does not continue and no other process runs; final procedures do
// run (llg: an error on stderr, exit status 1). It is the wait that starts
// on a null event: an event variable that becomes null during a wait leaves
// the wait as it is. Triggering a null event does nothing.
//
// IEEE 1800-2009 15.5.5.2 (SystemVerilog-1800-2009.txt L20912-20913):
//   "Triggering a null event shall have no effect. The outcome of waiting on
//   a null event is undefined, and implementations can issue a run-time
//   warning."
//
// The text leaves the outcome open; llg follows the industry-standard
// behaviour.
`timescale 1ns / 1ns
module tb;
  event empty = null;
  event named;
  event again;

  initial begin
    again = named;
    #1 named = null;
  end

  initial begin
    @named $display("woken at %0t", $time);
  end

  initial begin
    -> empty;
    ->> empty;
    #2 -> again;
    #1 $display("waiting at %0t", $time);
    @empty;
    $display("went on");
  end

  initial #4 $display("not reached");

  final $display("final at %0t", $time);
endmodule
