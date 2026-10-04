// IEEE 1800-2009 11.11: a declaration in one block is not visible in a sibling.
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
  initial begin
    begin : declares
      bind + function T addt(T, T);
    end
    begin : uses
      c = a + b;
    end
  end
endmodule
