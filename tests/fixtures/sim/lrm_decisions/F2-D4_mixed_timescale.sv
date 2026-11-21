// Decision F2-D4: a design in which some design elements have a time scale
// and others do not is an error, reported before simulation. Here `sub` has
// no time scale (no `timescale precedes it, it declares no timeunit or
// timeprecision) and `tb` has 1us/1ns, so the design must not run and prints
// nothing. llg names both elements in the error. A default time scale given
// on the command line (llg `--timescale`, as industry-standard tools offer)
// applies to the elements without one and makes such a design legal; this
// portable case gives none.
//
// IEEE 1800-2009 3.14.2.3 (SystemVerilog-1800-2009.txt L2962-2964): "The
// default time unit and precision are implementation-specific." / "It shall
// be an error if some design elements have a time unit and precision
// specified and others do not."
module sub;
  initial #1 $display("sub t=%0t", $realtime);
endmodule

`timescale 1us/1ns
module tb;
  sub u();
  initial begin
    #2 $display("tb t=%0t", $realtime);
    $finish;
  end
endmodule
