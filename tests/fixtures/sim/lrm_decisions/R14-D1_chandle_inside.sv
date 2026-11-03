// Decision R14-D1: `inside` accepts chandle operands and compares them with
// `==` (equal when both are null or both name the same C pointer), like
// class handles.
//
// IEEE 1800-2009 11.4.13 (SystemVerilog-1800-2009.txt L15363, L15376): "The
// expression on the left-hand side of the inside operator is any singular
// expression." "The inside operator uses the equality ( == ) operator on
// nonintegral expressions to perform the comparison." 6.14 (L5576-5577):
// "Only the following operators are valid on chandle variables: Equality
// (==), inequality (!=) with another chandle or with null". The text does
// not say whether `inside`, which is defined through `==`, is a use 6.14
// excludes; llg follows industry-standard behaviour and accepts it.
module tb;
  chandle p, q;
  initial begin
    $display("null in {null}: %0d", p inside {q});
    $display("null in {null, null}: %0d", p inside {q, null});
    $finish;
  end
endmodule
