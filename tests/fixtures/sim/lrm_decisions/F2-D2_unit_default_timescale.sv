// Decision F2-D2: a compilation unit with no timeunit or timeprecision
// declaration uses the default time unit (1ns in llg), whatever `timescale
// directive precedes its declarations. Its precision is the
// implementation-specific default. llg takes the finest precision that the
// design declares, but never one coarser than the default unit, so a $unit
// class does not make the global precision finer than the declared scales.
// Here every declared scale is 1ns/1ns: the class's #1.4 rounds to 1ns, and
// the time step, and with it the default %t unit, stays 1ns. The class's time
// unit is observed through the time its delays advance in the module, which
// counts in 1ns. ($printtimescale is not used: the form of its text is not
// specified.)
//
// IEEE 1800-2009 3.14.2.3 (SystemVerilog-1800-2009.txt L2956-2960): "The
// time unit of the compilation-unit scope can only be set by a timeunit
// declaration, not a `timescale directive. If it is not specified, then the
// default time unit shall be used. If a timeprecision is not specified in the
// current time scope, then the time precision shall be determined following
// the same precedence as with time units." L2962: "The default time unit and
// precision are implementation-specific." 3.14.3 (L2978-2980): "The global
// time precision, also called the simulation time unit, is the minimum of all
// the timeprecision statements, all the time precision arguments to timeunit
// declarations, and the smallest time precision argument of all the
// `timescale compiler directives in the design."
`timescale 1ns / 1ns
class C;
  task automatic run();
    #1.4;
    $display("class realtime=%0.3f", $realtime);
  endtask
  task automatic step();
    #1;
  endtask
endclass
module tb;
  C c;
  initial begin
    c = new;
    c.run();
    $display("tb realtime=%0.3f t=%0t", $realtime, $realtime);
    c.step();
    $display("class #1 moved the module to %0.3f", $realtime);
    $finish;
  end
endmodule
