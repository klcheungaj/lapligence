// A compound write whose class key calls a function: a located limit for any
// key type (docs/known_issues.md).
module tb;
  class K; endclass
  int a[K];
  K k;
  function K f(); return k; endfunction
  initial begin
    k = new;
    a[k] = 1;
    a[f()] += 1;
    $display("%0d", a[k]);
    $finish;
  end
endmodule
