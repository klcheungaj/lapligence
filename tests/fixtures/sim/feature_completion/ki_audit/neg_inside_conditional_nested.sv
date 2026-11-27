// Set membership items outside packed storage: a conditional expression
// with an element of a queue of queues as an operand.
module tb;
  int qq[$][$];
  int q[$] = '{9};
  int x = 2;
  bit c = 1;
  initial begin
    qq.push_back('{1, 2});
    $display("%0d", x inside {c ? qq[0] : q});
  end
endmodule
