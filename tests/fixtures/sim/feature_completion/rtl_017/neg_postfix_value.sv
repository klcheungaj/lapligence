// An overloaded postfix increment yields its old value, which the
// `a = inct(a)` form cannot provide; only its statement form is admitted.
typedef struct { int n; } T;
typedef struct { longint n; } W;
function automatic T addt(T a, T b);
  T r;
  r.n = a.n + b.n;
  return r;
endfunction
function automatic W addw(T a, T b);
  W r;
  r.n = a.n + b.n;
  return r;
endfunction
function automatic T inct(T a);
  T r;
  r.n = a.n + 1;
  return r;
endfunction
module tb;
  bind ++ function T inct(T);
  T a, b;
  initial b = a++;
endmodule
