// Documented limit (docs/known_issues.md): an overloaded update whose target
// selector has side effects binds its target once, which needs a
// packed-capacity target. A descriptor-sized (65,537-element) row keeps the
// limit in a for step exactly as in an expression statement.
module tb;
  localparam int N = 65537;
  typedef int vec_t [0:N-1];
  function automatic vec_t vinc(vec_t v);
    vec_t r;
    foreach (r[i]) r[i] = v[i] + 1;
    return r;
  endfunction
  bind ++ function vec_t vinc(vec_t);
  vec_t vv [2];
  int j, k;
  function automatic int next();
    k++;
    return k;
  endfunction
  initial begin
    k = -1;
    for (j = 0; j < 2; vv[next()]++) j++;
  end
endmodule
