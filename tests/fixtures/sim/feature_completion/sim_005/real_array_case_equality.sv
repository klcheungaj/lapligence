// SV 7.4.3, 11.4.5, decision R05-D4: `===` and `!==` over real and shortreal
// fixed arrays compare element by element, each element by its IEEE 754 bit
// pattern; `==` and `!=` keep the numeric comparison.
module tb;
  real ra[3], rb[3], rc[3];
  real m2a[2][2], m2b[2][2];
  shortreal sa[2], sb[2];
  real z, n;
  initial begin
    ra = '{1.0, 2.0, 3.0};
    rb = '{1.0, 2.0, 3.0};
    rc = '{1.0, 2.5, 3.0};
    $display("ra===rb %0d ra===rc %0d ra!==rc %0d ra!==rb %0d ra==rb %0d ra!=rc %0d",
             ra === rb, ra === rc, ra !== rc, ra !== rb, ra == rb, ra != rc);
    $display("slice %0d %0d", ra[0:1] === rc[0:1], ra[1:2] !== rc[1:2]);
    m2a = '{'{1.0, 2.0}, '{3.0, 4.0}};
    m2b = '{'{1.0, 2.0}, '{3.0, 4.5}};
    $display("rows %0d %0d %0d", m2a === m2a, m2a === m2b, m2a[0] === m2b[0]);
    sa = '{0.5, 0.25};
    sb = '{0.5, 0.25};
    $display("shortreal %0d %0d", sa === sb, sa !== sb);
    z = 0.0;
    ra = '{1.0, z, 3.0};
    rb = '{1.0, -z, 3.0};
    $display("signed zero == %0d === %0d", ra == rb, ra === rb);
    n = $bitstoreal(64'h7ff8000000000000);
    ra = '{1.0, n, 3.0};
    rb = '{1.0, n, 3.0};
    $display("nan == %0d === %0d !== %0d", ra == rb, ra === rb, ra !== rb);
    if (ra !== rc) $display("condition");
    $finish;
  end
endmodule
