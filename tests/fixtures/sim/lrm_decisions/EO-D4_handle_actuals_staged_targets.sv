// Decision EO-D4, actuals and staged targets: a computed handle that a class
// property is selected through is evaluated where its use starts, also for
// an element of an unpacked property passed as an actual: an input actual
// (`f2(bump(), ca[k].a[0])`) after the actuals to its left, and an output or
// inout actual of a task at its place among the actuals, its copy-out
// naming the same object (decision EO-D3). A simple assignment locates an
// element target after its right-hand side, also in a statement that names a
// queue or dynamic-array member of a record element of a queue, and also an
// element of such a member (`rq[k].q[0] = bump()`); a whole member target
// and the target of an assignment operator are located before it, also a
// packed member of such a record element (`rq[k].v = bump() + 1`).
//
// IEEE 1800-2009 13.5 (SystemVerilog-1800-2009.txt L19070): "The order of
//   evaluation of the expressions in the argument list is undefined."
// IEEE 1800-2009 13.5.2 (L19185-19187): "an inout argument is copied twice:
//   once from the actual into the argument when the subroutine is called and
//   once from the argument into the actual when the subroutine returns."
// IEEE 1800-2009 10.4.1 (L13026-13027): "The order of evaluation of the
//   variable_lvalue and the expression on the right-hand side is undefined if
//   a timing control is not specified."
class C;
  int a[4];
  string sa[2];
  int v;
  function int m(input int s, input int w);
    return s * 1000 + w + v;
  endfunction
endclass
module tb;
  typedef struct { int q[$]; int v; } r_t;
  r_t rq[$];
  int arr[4];
  C ca[2];
  C o;
  int k, x, n;
  function automatic int bump();
    k = 1;
    n++;
    return 1;
  endfunction
  function automatic int f2(input int s, input int v);
    return s * 100 + v;
  endfunction
  function automatic int fio(input int s, inout int v);
    v = v + 5;
    return s * 100 + v;
  endfunction
  task automatic tin(input int s, input int v);
    $display("tin %0d %0d", s, v);
  endtask
  task automatic tio(input int s, inout int v);
    v = v + 5;
  endtask
  task automatic tout(input int s, output int v);
    v = 66;
  endtask
  function automatic string fs(input int s, input string t);
    return $sformatf("%0d:%s", s, t);
  endfunction
  task automatic reset();
    r_t r;
    ca[0].a = '{10, 11, 12, 13}; ca[1].a = '{20, 21, 22, 23};
    ca[0].sa = '{"a0", "b0"}; ca[1].sa = '{"a1", "b1"};
    ca[0].v = 1; ca[1].v = 2;
    rq.delete();
    r.q = '{5, 6}; r.v = 7; rq.push_back(r);
    r.q = '{8}; r.v = 9; rq.push_back(r);
    arr = '{10, 11, 12, 13};
    k = 0; n = 0;
  endtask
  initial begin
    ca[0] = new; ca[1] = new; o = new; o.v = 0;
    reset(); x = f2(bump(), ca[k].a[0]);
    $display("A %0d", x);
    reset(); x = f2(bump(), ca[k].v);
    $display("B %0d", x);
    reset(); x = fio(bump(), ca[k].v);
    $display("C %0d %0d %0d", x, ca[0].v, ca[1].v);
    reset(); x = f2(bump(), ca[k].a[k]);
    $display("D %0d", x);
    reset(); x = bump() + f2(0, ca[k].a[0]);
    $display("E %0d", x);
    reset(); x = f2(ca[k].a[0], bump());
    $display("F %0d", x);
    reset(); tin(bump(), ca[k].a[0]);
    reset(); tio(bump(), ca[k].a[1]);
    $display("G %p %p", ca[0].a, ca[1].a);
    reset(); tout(bump(), ca[k].a[2]);
    $display("H %p %p", ca[0].a, ca[1].a);
    reset(); x = o.m(bump(), ca[k].a[3]);
    $display("I %0d", x);
    reset(); $display("J %s", fs(bump(), ca[k].sa[1]));
    reset(); $display("K %0d %0d", bump(), ca[k].a[0]);
    reset(); x = ca[k].m(bump(), ca[k].a[0]);
    $display("L %0d", x);
    reset(); tin(ca[k].a[1], bump());
    reset(); arr[k] = rq[0].q.size() + bump();
    $display("M %p", arr);
    reset(); ca[k].a[1] = rq[0].q[0] + bump();
    $display("N %p %p", ca[0].a, ca[1].a);
    reset(); rq[k].q[0] = bump() + 100;
    $display("O %p %p", rq[0].q, rq[1].q);
    reset(); rq[0].q[k] = bump() + 50;
    $display("P %p", rq[0].q);
    reset(); rq[k].q[0] = bump();
    $display("Q %p %p", rq[0].q, rq[1].q);
    reset(); rq[k].q = '{bump()};
    $display("R %p %p", rq[0].q, rq[1].q);
    reset(); rq[k].q[0] += bump();
    $display("S %p %p", rq[0].q, rq[1].q);
    reset(); rq[k].q.push_back(bump());
    $display("T %p %p", rq[0].q, rq[1].q);
    reset(); rq[k].v = bump() + 1;
    $display("U %0d %0d", rq[0].v, rq[1].v);
    reset(); rq[k].v += bump();
    $display("V %0d %0d", rq[0].v, rq[1].v);
    reset(); rq[k].v = rq[k].v + bump();
    $display("W %0d %0d", rq[0].v, rq[1].v);
    reset(); rq[bump()].v = bump() + 60;
    $display("X %0d %0d %0d", rq[0].v, rq[1].v, n);
    $finish;
  end
endmodule
