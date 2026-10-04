// IEEE 1800-2009 11.11: the bound function must agree with its prototype.
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
function automatic T addi(T a, int b);
  T r;
  r.n = a.n + b;
  return r;
endfunction
module tb;
  bind + function T addi(T, T);
  T a, b, c;
  initial c = a + b;
endmodule
