// Decision S45-D16: `!` before a select condition selects the bins of the
// condition's coverpoint that the condition leaves out, also when the
// condition names one bin: `! binsof(ca.lo) intersect {1}` is every bin of
// `ca` other than `lo` (here `hi`). The cross has Bc = 4 automatic bins (the
// `lo` row) and Bu = 1.
//
// IEEE 1800-2009 19.6 (SystemVerilog-1800-2009.txt L33113-33115): "Its
// negated form ! binsof( x ) intersect { y } denotes the bins of coverage
// point x whose values do not intersect the range given by y." The text only
// shows a whole coverpoint; it could also be read as the bins of `ca.lo`
// whose values do not intersect {1} (none).
module tb;
  bit [1:0] a, b;
  covergroup g;
    ca: coverpoint a { bins lo = {[0:1]}; bins hi = {[2:3]}; }
    cb: coverpoint b;
    ab: cross ca, cb { bins notlo = ! binsof(ca.lo) intersect {1}; }
  endgroup
  g c = new;
  initial begin
    a = 2; b = 0; c.sample();
    $display("after 2,0: %0.2f", c.ab.get_inst_coverage());
    $finish;
  end
endmodule
