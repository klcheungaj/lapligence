// Decision S25-D6: order of the reports of one time slot.
//
// IEEE 1800-2009 4.4.2.9 (SystemVerilog-1800-2009.txt L3210):
//   "$monitor, $strobe and other similar events are scheduled in the
//   Postponed region."
//
// The text gives no order among the events of the Postponed region. llg
// follows the industry-standard order: first the $monitor and $fmonitor lists
// report in registration order (a replaced $monitor moves to the end), then
// the pending $strobe and $fstrobe lines print in reverse issue order, the
// last issued first, across processes and files. llg previously printed the
// strobes first, in call order.
module tb;
  reg [3:0] a = 1;
  initial begin
    $strobe("strobe 1 a=%0d", a);
    $fmonitor(1, "fmonitor 1 a=%0d", a);
    $monitor("monitor a=%0d", a);
    $strobe("strobe 2 a=%0d", a);
    $fmonitor(1, "fmonitor 2 a=%0d", a);
    #1 a = 2;
    $strobe("strobe 3 a=%0d", a);
    #1 $finish;
  end
endmodule
