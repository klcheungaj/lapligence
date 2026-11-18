// An element of a container of containers as an `inside` set item: a located
// limit (docs/known_issues.md, set membership items outside packed storage).
module tb;
  int qq[$][$];
  int x = 2;
  initial begin
    qq.push_back('{1, 2, 3});
    $display("%0d", x inside {qq[0]});
    $finish;
  end
endmodule
