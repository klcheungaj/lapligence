// Decision S47-D12: reals are written with `%.17g`, which reads back to the
// same double, and vectors with every bit of their declared width.
//
// IEEE 1800-2009 21.7.2.1 (SystemVerilog-1800-2009.txt L38068-38070): "A real
// number is dumped using a %.16g printf() format. This preserves the
// precision of that number by outputting all 53 bits in the mantissa of a
// 64-bit IEEE 754 double-precision number." Sixteen significant digits do not
// identify every double; llg uses 17 for the stated purpose. L38090-38091:
// "Vector values appear in the shortest form possible"; a full-width value is
// read the same way (Table 21-9 left-extension).
// Compare S47-D12.vcd: `r0.33333333333333331` and `b00000001`.
module tb;
  real r;
  logic [7:0] w;
  initial begin
    r = 0.1;
    w = 8'h01;
    $dumpfile("S47-D12.vcd");
    $dumpvars(0, tb);
    #1 r = 1.0 / 3.0;
    $strobe("end of step 1: r=%.17g w=%b", r, w);
    #1 $finish;
  end
endmodule
