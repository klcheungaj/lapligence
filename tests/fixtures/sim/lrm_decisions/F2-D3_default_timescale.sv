// Decision F2-D3 (user decision): when no design element declares a time
// unit or precision, llg uses the default time unit 1ns and precision 1ps:
// `#1` advances 1000 ticks, `%t` prints 1000, `#1.6` rounds to 1.6ns and
// `$printtimescale` reports 1ns / 1ps. llg's `--timescale <unit>/<precision>`
// option (or `compile.timescale`) chooses another default; without it the
// `missing-timescale` lint warning names the elements that take 1ns/1ps.
// Industry-standard behaviour uses 1s / 1s there instead (`%t` prints 1,
// `#1.6` rounds to 2); llg keeps 1ns/1ps by design.
//
// IEEE 1800-2009 3.14.2.3 (SystemVerilog-1800-2009.txt L2962): "The default
// time unit and precision are implementation-specific."
module tb;
  initial begin
    #1 $display("t=%0t T=%t time=%0d real=%f", $time, $time, $time, $realtime);
    $display("%0t|%t", 1, 1);
    #1.6 $display("t=%0t real=%f", $time, $realtime);
    $timeformat(-9, 2, " ns", 10);
    $display("tf=%t", $realtime);
    $finish;
  end
endmodule
