// IEEE 1800-2009 11.11 L16473-16476: "The overload declaration allows the
// arithmetic operators to be applied to data types that are normally illegal
// for them, such as unpacked structures. It does not change the meaning of the
// operators for data types where it is legal to apply them." L16488-16489:
// "The arguments are matched, and the data type of the result is then
// checked." The clause's own example declares `bind + function float
// fcopyi(int); // unary +` although unary + is legal for int.
// Decision (llg reading): a prototype whose operand types are legal for the
// built-in operator but whose result type the built-in result cannot be
// assigned to (an unpacked record) is admitted; every legal use of the
// operator keeps its built-in meaning, so the sentinel results never appear.
module tb;
  typedef struct { int n; } T;
  function automatic T sentinel(int a, int b);
    T r;
    r.n = -1;
    return r;
  endfunction
  function automatic T sentinel_eq(T a, T b);
    T r;
    r.n = -2;
    return r;
  endfunction
  bind + function T sentinel(int, int);
  bind == function T sentinel_eq(T, T);
  int i, j;
  T s, t;
  initial begin
    i = 2;
    j = 3;
    s.n = 1;
    t.n = 1;
    $display("%0d %0d", i + j, s == t);
    $finish;
  end
endmodule
