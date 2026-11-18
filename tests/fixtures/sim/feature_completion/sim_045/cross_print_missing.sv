// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/cross_print_missing.sv
// SIM-045 (S46-D17): a negative option.cross_num_print_missing. A procedural
// write of one is ignored with a located run-time warning and the option
// keeps its value, at the cross and at the covergroup level; a positive write
// is stored. With DECL the definition sets a negative value: a located
// run-time error at the construction that ends the run (final procedures
// run, exit status 1). Expected values are derived in readme.md.
module tb;
  bit [1:0] x, y;
  int neg = -1;
  covergroup g(int n);
    option.cross_num_print_missing = 3;
    cp: coverpoint x;
    cq: coverpoint y;
`ifdef DECL
    xc: cross cp, cq { option.cross_num_print_missing = n; }
`else
    xc: cross cp, cq { option.cross_num_print_missing = 2; }
`endif
  endgroup
  g a;
  initial begin
    $display("before new");
    a = new(-4);
    $display("decl group=%0d xc=%0d", a.option.cross_num_print_missing,
             a.xc.option.cross_num_print_missing);
    a.xc.option.cross_num_print_missing = neg;
    $display("xc negative write=%0d", a.xc.option.cross_num_print_missing);
    a.option.cross_num_print_missing = -2;
    $display("group negative write=%0d", a.option.cross_num_print_missing);
    a.xc.option.cross_num_print_missing = 5;
    $display("xc positive write=%0d", a.xc.option.cross_num_print_missing);
    a.xc.option.cross_num_print_missing = 0;
    $display("xc zero write=%0d", a.xc.option.cross_num_print_missing);
    $finish;
  end
  final $display("final");
endmodule
