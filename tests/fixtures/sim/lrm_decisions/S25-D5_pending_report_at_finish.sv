// Decision S25-D5: postponed output still pending when $finish executes.
//
// IEEE 1800-2009 20.2 (SystemVerilog-1800-2009.txt L34119):
//   "The $finish system task causes the simulator to exit and pass control
//   back to the host operating system."
// IEEE 1800-2009 21.2.2 (L36482-36484): "$strobe ... end of the current
//   simulation time ... just before simulation time is advanced."
//
// The text does not say whether the Postponed region of the time slot that
// executes $finish still runs. llg exits at $finish and prints neither the
// pending $strobe nor the pending $monitor report of that slot, so the
// output ends with "before finish". A simulator that completes the slot would
// also print "pending strobe a=2" and "mon a=2".
module tb;
  reg [3:0] a = 1;
  initial begin
    $strobe("strobe a=%0d", a);
    $monitor("mon a=%0d", a);
    #1 a = 2;
    $strobe("pending strobe a=%0d", a);
    $display("before finish");
    $finish;
  end
endmodule
