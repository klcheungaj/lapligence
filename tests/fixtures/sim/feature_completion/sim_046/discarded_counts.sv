// SIM-046 regression (found by the FIN-002 scenarios): a coverage query
// whose result is discarded with a void cast still stores the covered and
// total bin numbers into its two arguments (19.8), for a covergroup, a
// coverpoint, a cross and the type query.
module tb;
  covergroup cg with function sample (bit [1:0] who, bit pass);
    option.per_instance = 1;
    cp_who: coverpoint who {bins none = {0}; bins one = {1}; bins two = {2};}
    cp_pass: coverpoint pass;
    x: cross cp_who, cp_pass;
  endgroup

  cg cov = new();
  int covered;
  int total;
  real kept;

  task clear;
    covered = -1;
    total   = -1;
  endtask

  initial begin
    cov.sample(2'd1, 1'b1);
    cov.sample(2'd2, 1'b0);
    clear;
    void'(cov.cp_who.get_inst_coverage(covered, total));
    $display("point, void cast: %0d/%0d", covered, total);
    clear;
    void'(cov.x.get_inst_coverage(covered, total));
    $display("cross, void cast: %0d/%0d", covered, total);
    clear;
    void'(cov.get_inst_coverage(covered, total));
    $display("group, void cast: %0d/%0d", covered, total);
    clear;
    void'(cov.cp_pass.get_coverage(covered, total));
    $display("type point, void cast: %0d/%0d", covered, total);
    clear;
    kept = cov.cp_who.get_inst_coverage(covered, total);
    $display("point, assigned: %0d/%0d %0.2f", covered, total, kept);
    $finish;
  end
endmodule
