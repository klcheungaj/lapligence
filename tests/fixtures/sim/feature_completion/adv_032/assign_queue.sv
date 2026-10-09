module tb;
  int items [$];
  initial begin
    assign items = {1, 2};
    #1 $display("%0d", items.size());
    $finish;
  end
endmodule
