// Decision S31-D2: each randcase weight is a self-determined expression, so
// it keeps the width of its own operands: a byte sum that carries out of
// 8 bits wraps inside the weight.
//
// IEEE 1800-2009 18.16 (SystemVerilog-1800-2009.txt L31346, L31349-31350):
//   "The precision of each weight expression is self-determined."
//   "In the example above, the first three weight expressions are computed
//   using 8-bit precision, and the fourth expression is computed using
//   12-bit precision."
//
// a + b with a = b = 128 is 0 in 8 bits, so the first branch has weight 0 and
// is never taken; computed in a 32-bit context it would be 256 and would be
// taken almost always.
module tb;
  byte unsigned a = 128;
  byte unsigned b = 128;
  int by_sum, by_one;
  initial begin
    repeat (300) begin
      randcase
        a + b : by_sum++;
        1 : by_one++;
      endcase
    end
    $display("sum branch: %0d", by_sum);
    $display("constant branch: %0d", by_one);
    $finish;
  end
endmodule
