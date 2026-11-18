// Decision S47-D15: on request (`llg --wave-deltas`, configuration key
// `waveform.deltas`) every change of a dumped variable is its own record, in
// execution order, so one time step can hold several records of a variable;
// by default a step holds at most one (S47-D2, S47-D14). In this mode no
// lowering step removes a write of a dumped model, so each update of a
// nonblocking assignment batch is recorded.
//
// IEEE 1800-2009 21.7.1.4 (SystemVerilog-1800-2009.txt L37887-37888): "When
// dumping is enabled, the value change dumper records the values of the
// variables that change during each time increment. Values of variables that
// do not change during a time increment are not dumped." The text does not
// say how many records an increment gives; the mode is an llg option for
// seeing delta activity, and the design's output does not depend on it.
// Compare S47-D15.vcd: by default `#1` holds q = 0010 and r = 0101 and `#2`
// nothing; with the mode `#1` holds q = 0001, r = 0101, q = 0010 and `#2`
// holds q = 0011, q = 0010.
module tb;
  logic [3:0] q = 4'd0;
  logic [3:0] r = 4'd0;
  initial begin
    $dumpfile("S47-D15.vcd");
    $dumpvars(0, tb);
    #1 q <= 4'd1;
    r <= 4'd5;
    q <= 4'd2;
    $strobe("1: q=%0d r=%0d", q, r);
    #1 q <= 4'd3;
    q <= 4'd2;
    $strobe("2: q=%0d r=%0d", q, r);
    #1 $finish;
  end
endmodule
