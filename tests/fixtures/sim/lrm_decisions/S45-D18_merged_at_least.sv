// Decision S45-D18: with type_option.merge_instances, a merged bin is
// covered when the sum of its counts reaches the largest option.at_least of
// the instances that have the bin. Here `lo` has 2 + 1 = 3 hits and `hi`
// 1 + 1 = 2 against at_least 3, so the type has 1 of 2 bins: 50.00.
//
// IEEE 1800-2009 19.11.3 (SystemVerilog-1800-2009.txt L33960-33962): "When
// bins overlap among instances, the cumulative coverage count of an
// overlapping bin is the sum of counts of that bin in all instances
// containing it." Table 19-1 (L33273-33274): "at_least=number 1 Minimum
// number of hits for each bin." option.at_least is an instance option and
// can differ between instances; the text does not say which value a merged
// bin uses (the smallest would give 100.00).
module tb;
  bit [1:0] a;
  covergroup g(int n);
    type_option.merge_instances = 1;
    option.get_inst_coverage = 1;
    option.at_least = n;
    coverpoint a { bins lo = {0}; bins hi = {1}; }
  endgroup
  g g1 = new(1), g2 = new(3);
  initial begin
    a = 0; g1.sample(); g1.sample(); g2.sample();
    a = 1; g1.sample(); g2.sample();
    $display("%0.2f %0.2f %0.2f", g::get_coverage(), g1.get_inst_coverage(), g2.get_inst_coverage());
    $finish;
  end
endmodule
