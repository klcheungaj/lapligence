// A run-time selected record element as a `ref` actual of a function called
// in an `if` condition is legal (SV 13.5.2). Below 16 cells llg binds it by
// lowering the statement once per element, which it does only for
// assignments, `return` and system-task statements; a condition rejects.
module tb;
  typedef struct { string s; int n; } el_t;
  typedef struct { el_t e[2]; } rec_t;
  rec_t o;
  int i = 1;
  function automatic int f(ref el_t r);
    return r.n;
  endfunction
  initial if (f(o.e[i]) == 0) $display("zero");
endmodule
