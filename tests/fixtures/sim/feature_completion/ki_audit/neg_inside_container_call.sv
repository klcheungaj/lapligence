// A container-result call as an `inside` set item: a located limit
// (docs/known_issues.md, set membership items outside packed storage).
module tb;
  typedef int q_t[$];
  int x = 2;
  function automatic q_t g();
    return '{1, 2, 3};
  endfunction
  initial begin
    $display("%0d", x inside {g()});
    $finish;
  end
endmodule
