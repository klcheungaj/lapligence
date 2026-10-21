// Decision S47-D11: an output error (the dump file cannot be created, or a
// write, flush or close fails) is reported once, dumping stops, the
// simulation runs to its end and the run exits with a failure status.
//
// IEEE 1800-2009 21.7.1.1 (SystemVerilog-1800-2009.txt L37757-37758): "a
// character string that names the file to be opened." The text does not say
// what happens when the file cannot be opened or written.
// llg prints "llg: waveform: cannot open `S47-D11-missing/dump.vcd`: ..." on
// stderr and exits 1 after printing both lines. The notice is the simulator's
// own report, not design output, and other simulators print it on stdout or
// stderr in their own wording; the design's output is the two `time` lines,
// which every simulator prints.
module tb;
  logic v;
  initial begin
    v = 1'b0;
    $dumpfile("S47-D11-missing/dump.vcd");
    $dumpvars(0, tb);
    #1 v = 1'b1;
    $display("time 1: v=%b", v);
    #1 $display("time 2: v=%b", v);
    $finish;
  end
endmodule
