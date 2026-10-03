// SV2009 6.20, 6.21, 10.5, 26.2-26.3: static declaration initializers run once
// before any process. A declaration precedes its simple references, so within
// a scope each initializer observes every earlier declaration's initial value,
// including values produced by zero-time function calls. Across scopes the
// language leaves the order open; the owner policy initializes a declaration
// after every static declaration its value reads (directly or through called
// functions), which matches declaration order whenever that is defined.
package base_pkg;
  localparam int K = 4;
  int a = 3;
  function automatic int twice(int v);
    return v * 2;
  endfunction
  int b = twice(a);
  int c = b + 1;
  int d = c * K;
  typedef struct packed { logic [3:0] hi; logic [3:0] lo; } pair_t;
  localparam pair_t PAIR = '{4'h1, 4'h2};
  pair_t p = '{PAIR.lo, PAIR.hi};
endpackage

package user_pkg;
  import base_pkg::*;
  int e = d + a;
  function int get_e();
    return e;
  endfunction
  int calls = 0;
  function automatic int next_call();
    calls++;
    return calls;
  endfunction
  int first_call = next_call();
  int second_call = next_call();
endpackage

module tb;
  import user_pkg::*;
  int m1 = get_e();
  int m2 = m1 + base_pkg::c;
  base_pkg::pair_t mp = base_pkg::p;
  initial begin
    $display("base %0d %0d %0d %0d %h", base_pkg::a, base_pkg::b, base_pkg::c, base_pkg::d, base_pkg::p);
    $display("user %0d %0d %0d %0d", e, first_call, second_call, calls);
    $display("module %0d %0d %h", m1, m2, mp);
    $finish(0);
  end
endmodule
