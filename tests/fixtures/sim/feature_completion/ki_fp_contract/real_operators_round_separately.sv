// Each real operator rounds its result to a double before the next operator
// uses it (IEEE 1800-2009 6.12: "the same as a C double", and note 10:
// "represented as described by IEEE Std 754"), so a multiply followed by an
// add is not one fused multiply-add. With
// a = 1 + 2**-30 and b = 1 - 2**-30 the exact product is 1 - 2**-60, which
// rounds to 1.0; adding -1.0 then gives 0.0. A fused multiply-add keeps the
// exact product and gives -2**-60 (bits bc30000000000000).
module tb;
  real e, a, b, c, one, r, q;
  assign q = a * b + c;

  function automatic real mul_add(input real x, input real y, input real z);
    return x * y + z;
  endfunction

  initial begin
    e = 1.0 / 1073741824.0;
    a = 1.0 + e;
    b = 1.0 - e;
    c = -1.0;
    one = 1.0;
    r = a * b;
    $display("product %h", $realtobits(r));
    r = a * b + c;
    $display("mul_add %h", $realtobits(r));
    r = c + a * b;
    $display("add_mul %h", $realtobits(r));
    r = a * b - one;
    $display("mul_sub %h", $realtobits(r));
    r = one - a * b;
    $display("sub_mul %h", $realtobits(r));
    r = -(a * b) + one;
    $display("neg_mul_add %h", $realtobits(r));
    $display("function %h", $realtobits(mul_add(a, b, c)));
    #1;
    $display("continuous %h", $realtobits(q));
    $finish;
  end
endmodule
