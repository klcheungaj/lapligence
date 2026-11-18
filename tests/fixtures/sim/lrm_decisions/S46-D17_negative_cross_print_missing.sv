// Decision S46-D17: a procedural write of a negative
// option.cross_num_print_missing is ignored (llg reports a run-time warning
// at the write): the option keeps its value, at the cross and at the
// covergroup level, and the run goes on. A negative value in the definition
// is a run-time error at the construction that ends the run. The option is a
// number of cross products (Table 19-1); the text does not say what a
// negative one does. llg follows industry-standard behaviour.
//
// IEEE 1800-2009 19.7 Table 19-1 (SystemVerilog-1800-2009.txt L33282-33283):
// "cross_num_print_missing= number 0 Number of missing (not covered) cross
// product bins that shall be saved to the coverage database and printed in
// the coverage report." 19.7 (L33343-33344): "Other instance-specific
// options can be assigned procedurally after a covergroup has been
// instantiated."
module tb;
  bit [1:0] x, y;
  int neg = -1;
  covergroup g;
    option.cross_num_print_missing = 3;
    cp: coverpoint x;
    cq: coverpoint y;
    xc: cross cp, cq { option.cross_num_print_missing = 2; }
  endgroup
  g a;
  initial begin
    a = new;
    $display("decl group=%0d xc=%0d", a.option.cross_num_print_missing,
             a.xc.option.cross_num_print_missing);
    a.xc.option.cross_num_print_missing = neg;
    $display("xc negative write=%0d", a.xc.option.cross_num_print_missing);
    a.option.cross_num_print_missing = -2;
    $display("group negative write=%0d", a.option.cross_num_print_missing);
    $finish;
  end
  final $display("final");
endmodule
