// Decision S45-D9: a sample the coverpoint's iff disables, and stop(), end
// the transition sequences in progress: (1 => 2) does not match 1, a
// disabled sample, 2, nor 1, stop(), start(), 2. A bin's own iff guard only
// decides whether a completed sequence counts.
//
// IEEE 1800-2009 19.5 (SystemVerilog-1800-2009.txt L32384-32385): "If the
// guard expression evaluates to false at a sampling point, the coverage
// point is ignored." L32433-32434: "If the expression is false at a sampling
// point, the count for the bin is not incremented." 19.8 Table 19-5 L33565:
// stop() "Stops collecting coverage". 19.5.1 L32543-32544: value1 => value2
// "represents the value of coverage point at two successive sample points".
// The text does not say whether an ignored sample or a stop interrupts a
// transition; llg ends the sequences in progress.
module tb;
  covergroup g with function sample(bit [3:0] x, bit en);
    cp: coverpoint x iff (en) { bins t = (1 => 2); }
    guarded: coverpoint x { bins t = (1 => 2) iff (en); }
  endgroup
  g skip = new, stopped = new;
  initial begin
    skip.sample(1, 1);
    skip.sample(5, 0);
    skip.sample(2, 1);
    $display("skipped: %0.2f %0.2f", skip.cp.get_inst_coverage(), skip.guarded.get_inst_coverage());
    skip.sample(1, 0);
    skip.sample(2, 1);
    $display("guard at the end: %0.2f %0.2f", skip.cp.get_inst_coverage(),
             skip.guarded.get_inst_coverage());
    stopped.sample(1, 1);
    stopped.stop();
    stopped.start();
    stopped.sample(2, 1);
    $display("stopped: %0.2f", stopped.cp.get_inst_coverage());
    stopped.sample(1, 1);
    stopped.sample(2, 1);
    $display("again: %0.2f", stopped.cp.get_inst_coverage());
    $finish;
  end
endmodule
