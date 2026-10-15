// Decision S47-D1: the dump starts at the end of the time step that executed
// $dumpvars, so its $dumpvars checkpoint holds the step's final values. Here
// the checkpoint at time 0 shows v = 2, not the value 1 the variable had when
// $dumpvars ran.
//
// IEEE 1800-2009 21.7.1.3 (SystemVerilog-1800-2009.txt L37832-37833):
// "Executing the $dumpvars task causes the value change dumping to start at
// the end of the current simulation time unit."
// Compare S47-D1.vcd: `#0 $dumpvars b0010 ... $end`, then `#1 b0011`.
module tb;
  logic [3:0] v;
  initial begin
    v = 4'h1;
    $dumpfile("S47-D1.vcd");
    $dumpvars(0, tb);
    v = 4'h2;
    $strobe("end of step 0: v=%h", v);
    #1 v = 4'h3;
    $strobe("end of step 1: v=%h", v);
    #1 $finish;
  end
endmodule
