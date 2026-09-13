module tb;
  integer level;

  initial begin
    level = 0;
    $finish(level);
    $display("CHECK: after");
  end
endmodule
