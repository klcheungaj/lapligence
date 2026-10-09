`delay_mode_zero
module tb;
  initial begin
    $display("no-op if accepted");
    $finish;
  end
endmodule
