// SV 16.9.3: outside an assertion, inferred clock or default clocking a
// clocking event is required.
module tb;
  logic v = 1'b0;
  initial begin
    #1 $display("%b", $rose(v));
    $finish;
  end
endmodule
