// Decision S45-D2: a coverpoint whose only bins declaration is a `default`
// bin has no automatic bins; it has no bin that counts toward coverage, so it
// does not contribute to its covergroup and returns 0.0 itself.
//
// IEEE 1800-2009 19.5 (SystemVerilog-1800-2009.txt L32409-32411): "If the
// bins are not explicitly defined, they are automatically created by
// SystemVerilog." L32436-32438: "The default specification defines a bin
// that is associated with none of the defined value bins. ... However, the
// coverage calculation for a coverage point shall not take into account the
// coverage captured by the default bin." 19.11.1 L33862-33866: "If none of
// the bins have an associated value or transition, the denominator of the
// coverage calculation is zero. In this case: a) The coverpoint does not
// contribute to the coverage computation ... b) If the coverpoint's weight is
// non-zero, a value of 0.0 is returned". llg reads `bins d = default` as an
// explicit bins definition, so no automatic bins are created.
module tb;
  bit [3:0] a;
  covergroup g;
    only_default: coverpoint a { bins d = default; }
    one: coverpoint a { bins x = {1}; }
  endgroup
  g c = new;
  initial begin
    a = 4'd3;
    c.sample();
    $display("%0.2f %0.2f", c.only_default.get_inst_coverage(), c.get_inst_coverage());
    a = 4'd1;
    c.sample();
    $display("%0.2f %0.2f", c.only_default.get_inst_coverage(), c.get_inst_coverage());
    $finish;
  end
endmodule
