// Decision S27-D18: an associative array with a wildcard index is not a
// memory_name; the call is rejected when the design is compiled.
//
// IEEE 1800-2009 21.4.1 (SystemVerilog-1800-2009.txt L37359):
//   "When loading associative arrays, indices shall be of integral types."
// IEEE 1800-2009 7.8.1 (L8639-8642):
//   "The array may be indexed by any integral expression. Because the index
//   expressions may be of different sizes, the same numerical value can have
//   multiple representations, each of a different size. SystemVerilog
//   resolves this ambiguity by removing the leading zeros and computing the
//   minimal length and using that representation for the value."
//
// A wildcard index accepts integral expressions but declares no index type,
// so a file address has no type to be cast to (S27-D16); llg rejects it (an
// llg choice). The expected output is empty: the design does not compile.
module tb;
  logic [7:0] table_ [*];
  initial begin
    $readmemh("s27_d18.mem", table_);
    $finish;
  end
endmodule
