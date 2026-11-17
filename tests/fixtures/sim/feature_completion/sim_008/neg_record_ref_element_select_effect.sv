// Left to right, `bump()` runs before the index of `o.e[i]` is read, so the
// call binds the element after the one `i` names when the statement starts.
// Below 16 cells llg chooses the element when the statement starts, so an
// earlier operand that may write the index rejects instead (SIM-008).
module tb;
  typedef struct { string s; int n; } el_t;
  typedef struct { el_t e[3]; } rec_t;
  rec_t o;
  int i = 0;
  int x;
  function automatic int bump();
    i = i + 1;
    return 0;
  endfunction
  function automatic int f(ref el_t r);
    r.n = r.n + 10;
    return r.n;
  endfunction
  initial x = bump() + f(o.e[i]);
endmodule
