module tb;
  initial begin
    $display("late=%0d", $unit::late_value);
    $finish(0);
  end
endmodule
int late_value = 7;
