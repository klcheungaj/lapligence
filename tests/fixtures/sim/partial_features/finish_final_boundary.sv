module tb;
  initial $finish(0);

  final begin
    $display("CHECK: final one");
    $finish(0);
    $display("CHECK: final one after");
  end

  final $display("CHECK: final two");
endmodule
