// Decision S25-D3: a monitored variable that changes and returns to its
// printed value inside one time slot, and an expression whose operands change
// but whose value does not, produce no report.
//
// IEEE 1800-2009 21.2.3 (SystemVerilog-1800-2009.txt L36516-36519, L36530-36531):
//   "each time a variable or an expression in the argument list changes
//   value ... the entire argument list is displayed at the end of the time
//   step as if reported by the $display task. If two or more arguments
//   change value at the same time, only one display is produced that shows
//   the new values."
//
// The text speaks of a change of value and reports at the end of the time
// step; llg compares the settled values at the end of the slot with the
// printed ones. A simulator that reports every value-change event would also
// print "q=0" at time 1 and "sum=5" at time 5.
module tb;
  reg [3:0] q = 0;
  reg [3:0] x = 2, y = 3;
  initial begin
    $monitor("q=%0d", q);
    #1 q = 5; q = 0;
    #1 q = 3;
    #1 $monitor("sum=%0d", x + y);
    #1;
    #1 x = 3; y = 2;
    #1 x = 1;
    #1 $finish;
  end
endmodule
