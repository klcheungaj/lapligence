// An overloaded compound assignment reads and writes its target separately,
// so a target whose selection has side effects is rejected.
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
  T arr [0:3];
  T b;
  int k;
  function automatic int next();
    k++;
    return k;
  endfunction
  initial arr[next()] += b;
endmodule
