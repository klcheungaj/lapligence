// Decision S45-D17: option.detect_overlap compares the values that two
// `bins` declarations of a coverpoint keep after ignore_bins and
// illegal_bins remove theirs, and for transition bins whether one sequence
// of samples completes both. llg warns once per pair of declarations when
// it builds the bin layout, so instances whose bins do not depend on
// covergroup arguments share one warning. Here `lo` and
// `mid` share 1; `mid` and `top` would share 2, but 2 is ignored; `s` and
// `l` share no sequence (a prefix is not an overlap); `r` and `r2` share
// 0 => 1 => 1; `v[]` and `w` share 3. Counting is unchanged.
//
// IEEE 1800-2009 19.7 Table 19-1 (SystemVerilog-1800-2009.txt
// L33276-33277): "detect_overlap=boolean 0 When true, a warning is issued
// if there is an overlap between the range list (or transition list) of two
// bins of a coverpoint." The text does not say whether values removed by
// ignore_bins or illegal_bins still overlap, what an overlap of two
// transition lists is, or when the warning is issued.
// get_inst_coverage() reads instance data, which option.per_instance = 1
// requires: 19.7 Table 19-1 (L33285-33289) "When true, coverage information
// for this covergroup instance shall be saved in the coverage database ...
// When false, implementations are not required to save instance-specific
// information."
module tb;
  bit [1:0] x;
  covergroup g;
    option.per_instance = 1;
    option.detect_overlap = 1;
    c1: coverpoint x { bins lo = {[0:1]}; bins mid = {[1:2]}; bins top = {[2:3]}; ignore_bins skip = {2}; }
    c2: coverpoint x { bins s = (1 => 2); bins l = (1 => 2 => 3); bins r = (0 => 1 [*2]); bins r2 = (0 => 1 => 1); }
    c3: coverpoint x { bins v[] = {[0:3]}; bins w = {3}; }
  endgroup
  g g1 = new, g2 = new;
  initial begin
    x = 0; g1.sample();
    x = 1; g1.sample();
    x = 1; g1.sample();
    $display("%0.2f %0.2f %0.2f", g1.c1.get_inst_coverage(), g1.c2.get_inst_coverage(),
             g1.c3.get_inst_coverage());
    $finish;
  end
endmodule
