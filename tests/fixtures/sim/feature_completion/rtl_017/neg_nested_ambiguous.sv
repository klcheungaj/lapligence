// IEEE 1800-2009 11.11: a nested operand has no single expected type, so
// prototypes that differ only in their result type need a cast there.
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
  bind + function T addt(T, T);
  bind + function W addw(T, T);
  T a, b, c;
  initial c = a + b + a;
endmodule
