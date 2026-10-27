// Decision A20-D13: the flags of a delay record. With mtm_flag a read gives
// every delay as min, typ and max, all equal to the one value simulation
// uses; a write takes the operand the model's min:typ:max selection uses
// (typical by default). With append_flag a write adds its values to the
// current ones; a delay never drops below 0. With pulsere_flag a module
// path reads and writes its delays with their reject and error limits; a
// primitive reads reject and error limits equal to its delay (its delays
// are inertial) and a write of pulse limits to it is a vpiError. A write of
// a path delay without pulsere_flag leaves its pulse limits as they were: a
// later pulse is filtered by the old limits. A negative absolute delay of a
// path or primitive is a vpiError and changes nothing.
//
// IEEE 1800-2009 38.32 (SystemVerilog-1800-2009.txt L63958-L63961): "The VPI
// routine vpi_put_delays() shall set the delays or timing limits of an
// object as indicated in the delay_p structure. [...] If only the delay
// changes and not the pulse limits, the pulse limits shall retain the values
// they had before the delays where altered."; 38.10 L62232-L62234:
// "PLI_INT32 mtm_flag; /* true for mtm values */ PLI_INT32 append_flag; /*
// true for append */ PLI_INT32 pulsere_flag; /* true for pulsere values */"
// and Table 38-2. What append adds to, which mtm operand a write takes, the
// pulse limits of a primitive and the refusal of negative delays are llg
// choices.
//
// Build A20-D13_delay_flags.c as a VPI application (llg: static archive
// given to --dpi-lib; `task $d13(input integer stage)` declared as a system
// task).
`timescale 1ns/1ns
module dut (input a, output y);
  buf ib (y, a);
  specify
    (a => y) = (6, 4);
  endspecify
endmodule

module tb;
  reg a = 1'b0;
  wire y, g;
  dut u (.a(a), .y(y));
  buf #(2:3:4, 5:6:7) gb (g, a);
  always @(y) $display("%0d y=%b", $time, y);
  always @(g) $display("%0d g=%b", $time, g);
  initial begin
    #1 $d13(0);
    // A 5 ns pulse: the path delays are now 9 ns, the reject limit of the
    // fall that ends the pulse still 4 ns.
    #9 a = 1'b1;
    #5 a = 1'b0;
    #15 $d13(1);
    #10 a = 1'b1;
    #5 a = 1'b0;
    #20 $finish;
  end
endmodule
