// Decision R05-D4: `===` and `!==` on fixed arrays of real or shortreal
// elements compare the arrays element by element, each element by its IEEE
// 754 bit pattern as for real members of other aggregates (R05-D3): equal
// reals match, 0.0 does not match -0.0, and a NaN matches the same NaN.
//
// IEEE 1800-2009 7.4.3 (SystemVerilog-1800-2009.txt L8089, L8095): "The
// following operations can be performed on all arrays, packed or unpacked."
// "Equality operations on the array or slice of the array, e.g., A==B,
// A[i:j] != B[i:j]". 11.4.5 L14775: "These four operators compare
// operands bit for bit." Table 11-1 (L14254-14255) lists the operand types of
// `===` and `!==` as "any except real and shortreal"; an array of reals is not
// itself a real operand, and the text gives no rule for its elements. llg
// follows industry-standard behaviour.
module tb;
  real ra[3], rb[3], rc[3];
  real m2a[2][2], m2b[2][2];
  shortreal sa[2], sb[2];
  real z, n;
  initial begin
    ra = '{1.0, 2.0, 3.0};
    rb = '{1.0, 2.0, 3.0};
    rc = '{1.0, 2.5, 3.0};
    $display("whole %0d %0d %0d %0d", ra === rb, ra === rc, ra !== rc, ra !== rb);
    m2a = '{'{1.0, 2.0}, '{3.0, 4.0}};
    m2b = '{'{1.0, 2.0}, '{3.0, 4.5}};
    $display("rows %0d %0d", m2a === m2b, m2a[0] === m2b[0]);
    sa = '{0.5, 0.25};
    sb = '{0.5, 0.25};
    $display("shortreal %0d %0d", sa === sb, sa !== sb);
    z = 0.0;
    ra = '{1.0, z, 3.0};
    rb = '{1.0, -z, 3.0};
    $display("signed zero %0d %0d", ra === rb, ra !== rb);
    n = $bitstoreal(64'h7ff8000000000000);
    ra = '{1.0, n, 3.0};
    rb = '{1.0, n, 3.0};
    $display("nan %0d %0d", ra === rb, ra !== rb);
    $finish;
  end
endmodule
