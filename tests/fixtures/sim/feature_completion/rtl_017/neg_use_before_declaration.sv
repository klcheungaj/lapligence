// IEEE 1800-2009 11.11: an overload declaration is visible only after it is
// declared, like a data declaration.
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
  T a, b, c;
  initial c = a + b;
  bind + function T addt(T, T);
endmodule
