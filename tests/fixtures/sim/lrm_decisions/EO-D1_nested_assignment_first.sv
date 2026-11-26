// Decision EO-D1: an assignment operator nested in an expression (`=` or
// `op=`) runs before the other operands of the full expression it belongs
// to: the operands a statement evaluates when it starts (both sides of an
// assignment, the actuals of a call or of a system task) or one evaluation
// of a loop condition. Several run left to right, and an assignment runs
// after the ones its own operands hold. One inside an operand that may be
// skipped (of `?:`, `&&` and `||`) runs in place, as do increments and
// decrements. The expression reads the value the assignment stacked.
//
// IEEE 1800-2009 11.4.2 (SystemVerilog-1800-2009.txt L14500): "The ordering
//   of assignment operations relative to any other operation within an
//   expression is undefined."
// IEEE 1800-2009 11.3.5 (L14429-14430): "Some operators (&&, ||, ->, and ?:)
//   shall use short circuit evaluation; in other words, some of their operand
//   expressions shall not be evaluated if their value is not required"
// IEEE 1800-2009 11.3.6 (L14463-14465): "a function that evaluates the
//   right-hand side, casts the right-hand side to the left-hand data type,
//   stacks it, updates the left-hand side, and returns the stacked value."
interface ifc;
  int a[4];
endinterface
class C;
  int a[4];
endclass
module tb;
  ifc i();
  ifc j();
  virtual ifc va[2];
  C ca[2];
  int arr[4];
  int k, x, y, z, n, c;
  real rv, rx;
  int log_q[$];
  function automatic int g(int v);
    log_q.push_back(v);
    return v;
  endfunction
  task automatic reset();
    arr = '{10, 20, 30, 40};
    k = 0; x = 0; y = 0; z = 0; n = 0; c = 0;
    rv = 0.5; rx = 0;
    va[0] = i; va[1] = j;
    i.a = '{10, 11, 12, 13}; j.a = '{20, 21, 22, 23};
    ca[0].a = '{10, 11, 12, 13}; ca[1].a = '{20, 21, 22, 23};
    log_q.delete();
  endtask
  initial begin
    ca[0] = new; ca[1] = new;
    reset(); x = arr[k] + ((k = 1) * 1) + arr[k];
    $display("A %0d", x);
    reset(); x = g(1) + g(2) + (y = g(3)) + (z = g(4));
    $display("B %0d %p", x, log_q);
    reset(); x = g(1) + (y = g(2) + (z = g(3)));
    $display("C %0d %p", x, log_q);
    reset(); arr[g(1)] = g(2) + (y = g(3));
    $display("D %p %p", arr, log_q);
    reset(); x = arr[k] + {(k = 1)} + arr[k];
    $display("E %0d", x);
    reset(); x = arr[k] + $clog2((k = 1) + 3) + arr[k];
    $display("F %0d", x);
    reset(); c = 1; x = arr[k] + ((k = 1) && c) + arr[k];
    $display("G %0d", x);
    reset(); x = g(1) + ((y = g(2)) ? g(3) : g(4)) + g(5);
    $display("H %0d %p", x, log_q);
    reset(); x = g(1) + (c ? 0 : (y = g(2))) + g(3);
    $display("I %0d %p", x, log_q);
    reset(); rx = rv + (rv = 2.0) + rv;
    $display("J %0.2f", rx);
    reset(); x = arr[k] + (k ^= 1) + arr[k];
    $display("K %0d", x);
    reset(); x = (arr[k] + (k = 1)) * (arr[k] + 0);
    $display("L %0d", x);
    reset(); while (arr[k] < (k = k + 1) * 15) n++;
    $display("M n=%0d k=%0d", n, k);
    reset(); $display("N %0d %0d", arr[k], (k = 2));
    reset(); x = va[k].a[0] + (k = 1) + va[k].a[0];
    $display("O %0d", x);
    reset(); x = ca[k].a[0] + (k = 1) + ca[k].a[0];
    $display("P %0d", x);
    reset(); x = arr[k++] + arr[k];
    $display("Q %0d k=%0d", x, k);
    $finish;
  end
endmodule
