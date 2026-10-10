// SIM-032: generate constructs are legal program items (IEEE 1800-2009 24.3,
// Syntax 24-1 program_generate_item); their initials are program initials.
program p #(parameter int N = 2);
  for (genvar i = 0; i < N; i++) begin : g
    initial #(i + 1) $display("gen %0d t=%0d", i, $time);
  end
  if (N > 1) begin : c
    initial $display("cond N=%0d", N);
  end
endprogram

module tb;
  p #(.N(3)) p0();
  initial #10 $display("module must not reach t=10");
endmodule
