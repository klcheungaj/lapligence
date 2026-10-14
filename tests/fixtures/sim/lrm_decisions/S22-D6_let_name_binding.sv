// Decision S22-D6: names in a let body that are not formal arguments, and
// names in a formal's default actual, resolve where the let is declared, so
// a caller's local of the same name is not captured. An actual argument is
// an expression of the caller and resolves in the caller's scope, even when
// it spells the same name as a free variable of the body. A formal shadows
// an outer name of the same spelling inside the body.
//
// IEEE 1800-2009 11.13 (SystemVerilog-1800-2009.txt L16778-16779):
//   "Variables used in a let that are not formal arguments to the let are
//   resolved according to the scoping rules from the scope in which the let
//   is declared."
// The clause's example a) expands the default `y = b` of `eq(r)` to
// `(m.r == m.b)`, the declaration-scope `b`; example b) binds `let y = x` to
// the module's `x` although the use site declares its own `x`.
module tb;
  int x = 10;
  int y = 1;
  let add_x(a) = a + x;
  let add_default(a, b = y) = a + b;
  let twice(x) = x * 2;
  initial begin
    automatic int x = 100;
    automatic int y = 50;
    $display("add_x(1)=%0d", add_x(1));
    $display("add_x(x)=%0d", add_x(x));
    $display("add_default(2)=%0d", add_default(2));
    $display("add_default(2, y)=%0d", add_default(2, y));
    $display("twice(3)=%0d", twice(3));
    $finish;
  end
endmodule
