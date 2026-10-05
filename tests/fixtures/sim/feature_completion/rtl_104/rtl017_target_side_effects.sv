// Formerly the RTL-017 negative `neg_target_side_effects`: an overloaded
// compound assignment evaluates its target, including a side-effecting index
// call, exactly once for both the read and the write (IEEE 1800-2009 11.11).
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
  initial begin
    foreach (arr[i]) arr[i].n = 10 * i;
    b.n = 5;
    arr[next()] += b;
    $display("%0d %0d %0d %0d k=%0d", arr[0].n, arr[1].n, arr[2].n, arr[3].n, k);
    $finish(0);
  end
endmodule
