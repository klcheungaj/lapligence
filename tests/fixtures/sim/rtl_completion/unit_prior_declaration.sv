int early_value = 7;
module tb;
  initial begin
    $display("early=%0d", $unit::early_value);
    $finish(0);
  end
endmodule
