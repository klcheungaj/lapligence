// IEEE 1800-2009 10.9.1: the last matching type key wins.
module tb;
  int values[0:1];
  int seed;
  initial begin
    seed = 7;
    values = '{int:seed, int:seed+1};
    if (values[0] !== 8 || values[1] !== 8) $fatal(1, "type precedence");
    $display("PASS r04_duplicate_type_keys");
    $finish;
  end
endmodule
