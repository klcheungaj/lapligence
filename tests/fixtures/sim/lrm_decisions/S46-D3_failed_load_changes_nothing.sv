// Decision S46-D3: $load_coverage_db of a file that does not exist (or is
// not a coverage database of this design) is a run-time error that changes
// no coverage; the simulation continues and ends with a failure status.
//
// IEEE 1800-2009 19.9 (SystemVerilog-1800-2009.txt L33685-33686):
// "$load_coverage_db ( filename ) loads from the given filename the
// cumulative coverage information for all coverage group types." The text is
// silent on a file that cannot be loaded; 40.3.2.4 (L66334-66335) makes the
// same situation an error for $coverage_merge: "If name does not exist or
// does not correspond to a coverage database from the same design, an error
// shall occur."
module tb;
  bit [1:0] a;
  covergroup g;
    coverpoint a { bins lo = {0}; bins hi = {1}; }
  endgroup
  g g1 = new;
  initial begin
    a = 0; g1.sample();
    $display("before %0.2f", g::get_coverage());
    $load_coverage_db("s46_d3_no_such_coverage_database");
    $display("after %0.2f", g::get_coverage());
    $finish;
  end
endmodule
