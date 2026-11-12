// SIM-046 --code-coverage: statement and toggle coverage collected from
// time 0 without any $coverage_* call (IEEE 1800-2009 40.3.2.1). The design
// names no coverage kind; the model is built for VPI (the $cov_report call
// site), so both kinds are available but stopped unless the invocation
// starts them: `--code-coverage` starts both, `--code-coverage=toggle` the
// toggle kind only, and `simulator.code_coverage = ["statement"]` in an
// llg.toml the statement kind only. The VPI application (option_kinds.c)
// prints the covered statements and bits of tb and tb.u at the end.
// Derivations: readme.md.
module leaf(input logic go);
  logic [1:0] n = 2'b00;
  always @(posedge go) begin
    n++;
    if (n > 2'd1) n = 2'd0;
  end
endmodule

module tb;
  logic go = 1'b0;
  leaf u(go);
  initial begin
    repeat (3) begin
      #1 go = 1'b1;
      #1 go = 1'b0;
    end
    #1 $cov_report;
    $finish;
  end
endmodule
