// SV2009 3.12.1, 6.21, 26.3, 26.6: merged-unit declarations initialize once;
// the re-exported package variable and the original are one object.
module tb;
  import reexport_ns::*;
  int u = unit_next();
  unit_pair_t pp = UNIT_P;
  initial begin
    shared = shared + 1;
    $display("%0d %0d %h %0d %0d %0d", u, unit_count, pp, mine, add_shared(1), base_ns::shared);
    $finish(0);
  end
endmodule
