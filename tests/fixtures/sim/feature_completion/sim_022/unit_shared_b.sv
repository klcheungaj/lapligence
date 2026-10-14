// SIM-022-A02: the second file reads the first file's compilation-unit
// declarations, legal only when both files form one compilation unit.
module tb;
  ma a();
  ma b();
  initial begin
    #1;
    $display("tb text=%s queue=%0d counter=%0d", unit_text, unit_queue.size(), unit_counter.value);
    $finish;
  end
endmodule
