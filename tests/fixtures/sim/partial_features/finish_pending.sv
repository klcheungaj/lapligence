module tb;
  reg value = 0;

  initial begin
    value <= 1;
    $display("CHECK: nba queued");
  end

  initial begin
    $display("CHECK: finish");
    $finish(0);
    $display("CHECK: finish after");
  end

  initial begin
    #10 $display("CHECK: timed");
  end

  final $display("CHECK: final value=%0d", value);
endmodule
