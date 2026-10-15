// Decision S47-D6: the $dumpoff checkpoint writes a real variable as `rNaN`;
// four-state variables are written as x.
//
// IEEE 1800-2009 21.7.1.3 (SystemVerilog-1800-2009.txt L37844-37845): "a
// checkpoint is made in which every selected variable is dumped as an x
// value." L38065 (21.7.2.1): "Value changes for real variables are specified
// by real numbers." A real number has no x; llg writes NaN.
// Compare S47-D6.vcd: `$dumpoff bx rNaN x $end` at time 1.
module tb;
  real r;
  logic b;
  logic [3:0] w;
  initial begin
    r = 1.5;
    b = 1'b1;
    w = 4'h9;
    $dumpfile("S47-D6.vcd");
    $dumpvars(0, tb);
    #1 $dumpoff;
    #1 $dumpon;
    $display("r=%.1f b=%b w=%h", r, b, w);
    #1 $finish;
  end
endmodule
