// Decision A20-D14: which objects have delays, and when a write takes
// effect. Module paths, timing checks and primitives have readable and
// writable delays; a continuous assignment has readable delays (rise, fall,
// turn-off; two declared delays give turn-off as the smaller) and a write to
// it is a vpiError. A primitive declared without a delay reads 0 and a
// write to it is a vpiError. A module has no delays (a vpiError). A net
// without a delay and an input port without module input port delays read
// 0; an output port has no delays (decisions A20-D17 and A20-D19). A write
// changes the transitions scheduled after it; a transition already
// scheduled keeps its time.
//
// IEEE 1800-2009 37.43 (SystemVerilog-1800-2009.txt L60294-L60295): the cont
// assign diagram lists "-> delay vpi_get_delays()" only; 37.31 (L59762-
// L59764) "-> delay vpi_get_delays() vpi_put_delays()" for primitives;
// 37.3.4 L57418-L57425: "Nets, primitives, module paths, timing checks, and
// continuous assignments can have delays specified within the SystemVerilog
// source code. [...] To access the actual delays being used by the tool,
// use the routine vpi_get_delays() on any of these objects."; 37.14 L58165
// "-> delay (mipd)" for ports. The delays of undelayed primitives and the
// time a write takes effect are llg choices.
//
// Build A20-D14_delay_objects.c as a VPI application (llg: static archive
// given to --dpi-lib; `task $d14(input integer stage)` declared as a system
// task).
`timescale 1ns/1ns
module dut (input a, output y);
  buf ib (y, a);
  specify
    (a => y) = 5;
  endspecify
endmodule

module tb;
  reg a = 1'b0;
  wire y, h, c;
  dut u (.a(a), .y(y));
  buf nb (h, a);
  assign #(1, 2) c = a;
  always @(y) $display("%0d y=%b", $time, y);
  initial begin
    #1 $d14(0);
    #9 a = 1'b1;
    // The rise of y is already scheduled for time 15.
    #2 $d14(1);
    #8 a = 1'b0;
    #10 $finish;
  end
endmodule
