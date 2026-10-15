// Decision S45-D4: a wildcard bins range whose bounds have X, Z or ? bits
// covers the union of the ranges of every expansion of its bounds: from the
// smallest value of the low bound to the largest value of the high bound.
//
// IEEE 1800-2009 19.5.6 (SystemVerilog-1800-2009.txt L32862-32863): "For
// wildcard bins, x and z values in b shall be treated as all possible 0 and 1
// values prior to applying these rules." 19.5.3 L32769-32770: "A wildcard
// bin definition causes all X, Z, or ? to be treated as wildcards for 0 or
// 1." The text only shows singleton patterns; llg reads a range as the union
// over all bound expansions.
module tb;
  bit [3:0] a;
  covergroup g;
    cp: coverpoint a {
      wildcard bins r = {[4'b01?1:4'b10?0]};
      bins other = {15};
    }
  endgroup
  g c = new;
  initial begin
    a = 4'd5;
    c.sample();
    $display("5: %0.2f", c.cp.get_inst_coverage());
    a = 4'd10;
    c.sample();
    $display("10: %0.2f", c.cp.get_inst_coverage());
    $finish;
  end
endmodule
