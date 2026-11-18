// Decision S47-D14: a variable whose end-of-step value equals the value last
// written for it (by a record or a checkpoint) has no record in that step,
// whether it changed and returned within one process turn, within one
// nonblocking assignment batch or in a later delta of the step. This holds for
// four-state values with x and z bits and for reals (compared bit for bit).
// The records of one time step are written in a fixed order of the
// variables, not in the order of the writes.
//
// IEEE 1800-2009 21.7.1.4 (SystemVerilog-1800-2009.txt L37887-37888): "When
// dumping is enabled, the value change dumper records the values of the
// variables that change during each time increment. Values of variables that
// do not change during a time increment are not dumped." A variable that ends
// the increment where it started has not changed over it. Industry-standard
// behaviour also writes no record for a value undone within one process turn
// or batch, but writes a record for each value a later delta sees (two
// records at `#2` below); llg writes at most one record per step (S47-D2).
// Compare S47-D14.vcd: the `#0` checkpoint, then only `#3` with v = 0110.
module tb;
  logic [3:0] v;
  logic [3:0] q = 4'd2;
  real r;
  initial begin
    v = 4'bx01z;
    r = 0.5;
    $dumpfile("S47-D14.vcd");
    $dumpvars(0, tb);
    #1 v = 4'b0000;
    v = 4'bx01z;
    r = 1.5;
    r = 0.5;
    q <= 4'd3;
    q <= 4'd2;
    $strobe("1: v=%b q=%0d r=%0.1f", v, q, r);
    #1 v = 4'b0110;
    #0 v = 4'bx01z;
    $strobe("2: v=%b", v);
    #1 v = 4'b0110;
    $strobe("3: v=%b", v);
    #1 $finish;
  end
endmodule
