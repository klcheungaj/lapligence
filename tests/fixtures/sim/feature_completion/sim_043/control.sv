// SIM-043 A03: vpi_control(vpiStop/vpiFinish) from callbacks (control.c)
// goes through the $stop/$finish lifecycle; end callbacks run once.
`timescale 1ns / 1ns
module tb;
  int steps = 0;
  initial begin
    #3 $display("[%0t] hdl: after the stop", $time);
    steps = 1;
    #4 $display("[%0t] hdl: active before the finish", $time);
    steps = 2;
    #20 $display("[%0t] hdl: unreachable", $time);
  end
  final $display("[%0t] final: steps=%0d", $time, steps);
endmodule
