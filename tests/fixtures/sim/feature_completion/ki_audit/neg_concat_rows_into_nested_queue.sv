// Remaining items of unpacked array concatenations: an array of rows as an
// item of a queue of queues.
module tb;
  int m[2][2] = '{'{1, 2}, '{3, 4}};
  int q[$] = '{5};
  int qq[$][$];
  initial begin
    qq = {m, q};
    $display("%p", qq);
  end
endmodule
