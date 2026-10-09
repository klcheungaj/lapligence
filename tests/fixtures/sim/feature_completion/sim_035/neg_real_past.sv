// SV 16.6.1 (SystemVerilog-1800-2009.txt L21575-21576): "The following types
// are not allowed: — Noninteger types (shortreal, real, and realtime)".
// 16.6 applies this to concurrent assertions; 16.9.3 allows the functions in
// procedural code without restating it. llg applies the same operand rule to
// a procedural $past of a real with an explicit clocking event (tool decision).
module tb;
  logic a = 1'b0;
  real r = 0.25;
  initial begin
    #1 a = 1'b1;
    #1 r = 2.5;
    #1 $display("%.2f", $past(r, 1, , @(posedge a)));
    $finish;
  end
endmodule
