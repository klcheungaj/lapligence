// Decision S25-D1: the monitor flag set by $monitoroff outlives the display
// list that was active when it was turned off; a $monitor issued while the
// flag is off stays silent until $monitoron.
//
// IEEE 1800-2009 21.2.3 (SystemVerilog-1800-2009.txt L36536-36541):
//   "The $monitoron and $monitoroff tasks control a monitor flag that
//   enables and disables the monitoring. Use $monitoroff to turn off the
//   flag and disable monitoring. The $monitoron system task can be used to
//   turn on the flag so that monitoring is enabled and the most recent call
//   to $monitor can resume its display. ... By default, the monitor flag is
//   turned on at the beginning of simulation."
//
// The text names one flag that only $monitoroff clears and only $monitoron
// sets, so llg keeps it off across the later $monitor call (it prints
// "C c=1" only after $monitoron). A simulator that re-enables monitoring in
// $monitor would also print "C c=3" at time 1 and "C c=1" at time 2.
module tb;
  reg [3:0] a = 1, c = 3;
  initial begin
    $monitor("A a=%0d", a);
    #1 $monitoroff;
    $monitor("C c=%0d", c);
    #1 c = 1;
    #1 $monitoron;
    #1 c = 2;
    #1 $finish;
  end
endmodule
