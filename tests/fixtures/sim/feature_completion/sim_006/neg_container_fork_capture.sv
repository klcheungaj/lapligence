module tb;
  task automatic t(input int q[$]);
    fork
      #1 $display("%0d", q.size());
    join
  endtask
  initial t('{1, 2});
endmodule
