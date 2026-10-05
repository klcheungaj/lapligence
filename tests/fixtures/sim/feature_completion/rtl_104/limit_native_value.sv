// Documented limit (docs/known_issues.md): an overloaded update whose value is
// used needs a packed-capacity target; a record with a string member has no
// such form yet. Its statement form keeps the ordinary assignment.
module tb;
  typedef struct { string s; int n; } s_t;
  function automatic s_t inc(s_t a);
    inc.s = {a.s, "+"};
    inc.n = a.n + 1;
  endfunction
  bind ++ function s_t inc(s_t);
  s_t x, y;
  initial begin
    x.n = 1;
    x++;
    y = x++;
  end
endmodule
