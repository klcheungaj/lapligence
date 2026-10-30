// Decision A20-D17: module input port delays (MIPD). An input or inout port
// and each of its port bits read and write the delays (rise, fall,
// turn-off) a change of the connected value takes to reach the port's net
// inside the module; they read 0 until written. A write takes 2 or 3
// delays (two give turn-off as the smaller), mtm_flag and append_flag as on
// a primitive; a write to a port sets every bit, a write to a port bit only
// that bit, and a read of a port gives its least significant bit. The delay
// is inertial: a pulse shorter than it is filtered. pulsere_flag, an output
// port and any other count are a vpiError and change nothing.
//
// IEEE 1800-2009 37.14 (SystemVerilog-1800-2009.txt L58165-L58167): the
// ports diagram draws "-> delay (mipd) vpi_get_delays() vpi_put_delays()";
// 37.3.4 L57418-L57419: "Additional delays may exist, such as module input
// port delays or inter-module path delays, that do not appear within the
// SystemVerilog source code."; 38.10 L62270-L62275 and 38.32 L64009-L64014
// list the legal delay counts of primitives, paths, timing checks and
// intermodule paths, not of ports. The count 2 or 3 is the count of an
// intermodule path, whose delays are those of its load port (decision
// A20-D18); some simulators refuse 2 although their diagnostic states 2 or
// 3. Inertial delays, the bit a port read gives and the refusal of pulse
// limits match industry-standard behaviour.
//
// Build A20-D17_port_delays.c as a VPI application (llg: static archive
// given to --dpi-lib; `task $d17(input integer stage)` declared as a system
// task).
`timescale 1ns/1ns
module snk (input i, input [1:0] v, output o);
  assign o = i;
  always @(i) $display("%0d i=%b", $time, i);
  always @(v) $display("%0d v=%b", $time, v);
endmodule

module tb;
  reg a = 1'b0;
  reg [1:0] vv = 2'b00;
  wire n, o;
  assign n = a;
  snk k (.i(n), .v(vv), .o(o));
  initial begin
    #1 $d17(0);
    #9 a = 1'b1;
    vv = 2'b11;
    #10 a = 1'b0;
    vv = 2'b00;
    #10 a = 1'b1;
    #2 a = 1'b0;
    #10 $d17(1);
    #10 $finish;
  end
endmodule
