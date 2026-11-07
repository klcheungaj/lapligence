// Decision S11-D7: an ambiguous condition of a conditional operator with class
// operands yields null, also when both operands are the same handle, and
// neither operand is evaluated. A condition with a known 1 bit among X bits is
// true. Some simulators return the second operand; llg follows the clause text
// because SV 11.4.11 states the result for class operands, and only its other
// case evaluates and combines both operands.
//
// IEEE 1800-2009 11.4.11 (SystemVerilog-1800-2009.txt L15211-15215): "For
// nonintegral and aggregate expressions, if cond_predicate evaluates to an
// ambiguous value, then: - If the first expression and the second expression
// are of a class data type and if the conditional operation is legal, then the
// resulting type is determined as defined above and the result is null.
// - Otherwise, both the first expression and second expression shall be
// evaluated, and their results shall be combined element by element."
class C;
  int a;
  function new(int x = 0); a = x; endfunction
endclass
module tb;
  C a = new(1), b = new(2), r;
  logic x;
  logic [1:0] two;
  function automatic C pick(C h);
    $display("pick %0d", h.a);
    return h;
  endfunction
  initial begin
    r = x ? a : b;
    $display("%0d", r == null);
    r = x ? null : a;
    $display("%0d", r == null);
    r = x ? a : a;
    $display("%0d", r == null);
    r = x ? pick(a) : pick(b);
    $display("%0d", r == null);
    x = 1'bz;
    r = x ? a : a;
    $display("%0d", r == null);
    two = 2'bx1;
    r = two ? a : b;
    $display("%0d", r.a);
    x = 1'b0;
    r = x ? a : b;
    $display("%0d", r.a);
  end
endmodule
