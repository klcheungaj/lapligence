// Formerly the RTL-017 negative `neg_postfix_value`: an overloaded postfix
// increment used as a value yields the target's old value (IEEE 1800-2009
// 11.11 with 11.4.2) and updates the target through the bound function.
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
  initial begin
    a.n = 41;
    b = a++;
    $display("%0d %0d", a.n, b.n);
    $finish(0);
  end
endmodule
