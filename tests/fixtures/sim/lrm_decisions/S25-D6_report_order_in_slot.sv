// Decision S25-D6: order of the reports of one time slot.
//
// IEEE 1800-2009 4.4.2.9 (SystemVerilog-1800-2009.txt L3210):
//   "$monitor, $strobe and other similar events are scheduled in the
//   Postponed region."
//
// The text gives no order among the events of the Postponed region. llg
// policy: strobes print in call order, then the $monitor and $fmonitor lists
// print in registration order. Every line of the output is llg policy; a
// simulator may order the lines differently, and the set of lines is the
// portable result.
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
