// Documented limit (docs/known_issues.md): an overloaded update whose target
// selector has side effects binds the target once, which needs a
// packed-capacity target; a record with a string member has no such form yet.
// Its value forms with side-effect-free targets run since SIM-021.
module tb;
  typedef struct { string s; int n; } s_t;
  function automatic s_t inc(s_t a);
    inc.s = {a.s, "+"};
    inc.n = a.n + 1;
  endfunction
  bind ++ function s_t inc(s_t);
  s_t x[2];
  int k;
  function automatic int next();
    k++;
    return k - 1;
  endfunction
  initial begin
    x[0].n = 1;
    x[next()]++;
  end
endmodule
