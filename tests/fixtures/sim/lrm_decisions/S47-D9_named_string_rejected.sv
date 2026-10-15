// Decision S47-D9 (negative): naming a variable that has no VCD encoding in
// $dumpvars is a compile error, not a selection of nothing. llg reports
// "`$dumpvars` argument `tb.label` in `tb` has no VCD/FST representation".
//
// IEEE 1800-2009 21.7.1.2 (SystemVerilog-1800-2009.txt L37790-37791): "These
// arguments can specify entire modules or individual variables within a
// module." Syntax 21-20 (L38022-38031) has no value format for a string.
// The expected output is empty because compilation fails.
module tb;
  string label;
  initial begin
    label = "x";
    $dumpvars(0, tb.label);
    $display("not reached");
    $finish;
  end
endmodule
