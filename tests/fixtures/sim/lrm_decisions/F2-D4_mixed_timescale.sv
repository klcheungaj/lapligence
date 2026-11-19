// Decision F2-D4: a design in which some design elements have a time scale
// and others do not is accepted. The elements without one take the default
// time scale (1ns/1ps, decision F2-D3) and llg reports them with its
// `missing-timescale` lint warning; the run continues. Here `sub` has no
// time scale (no `timescale precedes it), `tb` has 1us/1ns, so the global
// precision is 1ps: sub's #1 is 1ns (%t prints 1000 in 1ps units) and tb's
// #2 is 2us (2000000).
//
// IEEE 1800-2009 3.14.2.3 (SystemVerilog-1800-2009.txt L2962-2964): "The
// default time unit and precision are implementation-specific." / "It shall
// be an error if some design elements have a time unit and precision
// specified and others do not." llg departs from the second sentence (an
// error there would stop designs llg used to accept); industry-standard
// behaviour reports the error unless a default time scale is given on its
// command line.
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
