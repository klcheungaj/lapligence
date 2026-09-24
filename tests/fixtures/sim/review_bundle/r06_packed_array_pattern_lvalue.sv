// IEEE 1800-2009 10.9 permits positional deconstruction of packed arrays.
module tb;
  typedef logic [1:0][7:0] packed_pair_t;
  packed_pair_t source;
  logic [7:0] first, second;
  initial begin
    source = 16'h1234;
    packed_pair_t'{first, second} = source;
    if (first !== 8'h12 || second !== 8'h34)
      $fatal(1, "packed array deconstruction");
    $display("PASS r06_packed_array_pattern_lvalue");
    $finish;
  end
endmodule
