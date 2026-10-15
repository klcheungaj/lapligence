// Decision S46-D7: with type_option.merge_instances, the covered and total
// numbers of get_coverage(ref int, ref int) are those of the merged bins
// (the union of 19.11.3), and with merge_instances 0 they are summed over
// the instances. Here g1 hits lo and g2 hits lo and hi: unmerged 1 + 2 of
// 2 + 2 bins, merged 2 of 2.
//
// IEEE 1800-2009 19.8 (SystemVerilog-1800-2009.txt L33592-33595): "When
// get_coverage() is called on a coverpoint or cross, these values are
// aggregated numbers of bins from the same coverpoint or cross in all
// instances. When get_coverage() is called on a covergroup, these values are
// aggregated numbers of bins from all coverpoints and crosses in all
// instances." 19.11.3 (L33956-33957): "The values returned by
// get_coverage(ref int, ref int) are consistent with the weighted sum above
// when type_option.merge_instances is false." The text is silent on the
// values when merge_instances is true.
module tb;
  bit [1:0] a;
  int covered, total;
  real r;
  covergroup g;
    coverpoint a { bins lo = {0}; bins hi = {1}; }
  endgroup
  g g1 = new, g2 = new;
  initial begin
    a = 0; g1.sample(); g2.sample();
    a = 1; g2.sample();
    r = g::get_coverage(covered, total);
    $display("unmerged %0d %0d %0.2f", covered, total, r);
    g::type_option.merge_instances = 1;
    r = g::get_coverage(covered, total);
    $display("merged %0d %0d %0.2f", covered, total, r);
    $finish;
  end
endmodule
