// Class-keyed associative arrays have explicit gaps: the same find_index
// limit on an array indexed by chandles.
module tb;
  int a[chandle];
  chandle r[$];
  initial begin
    a[null] = 1;
    r = a.find_index with (item == 1);
    $display("%0d", r.size());
  end
endmodule
