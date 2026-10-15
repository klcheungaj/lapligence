// Decision S46-D2: $load_coverage_db adds the saved records to type
// coverage only: a saved instance joins the live instance with the same name
// (its bins are merged by name, 19.11.3), get_inst_coverage keeps reporting
// the live instance alone, and $get_coverage includes the loaded counts.
//
// Two runs in one directory: the first (no plusargs) samples g1 = 0 and
// saves s46_d2.db at the end; the second (+load) loads it and samples g1 = 1.
// Expected: first run "first 50.00 50.00"; second run "loaded 50.00 0.00",
// then "joined 100.00 50.00" (type: lo from the first run and hi from this
// one; instance: hi only) and "total 100.00".
//
// IEEE 1800-2009 19.9 (SystemVerilog-1800-2009.txt L33683-33686):
// "$set_coverage_db_name ( filename ) sets the filename of the coverage
// database into which coverage information is saved at the end of a
// simulation run. $load_coverage_db ( filename ) loads from the given
// filename the cumulative coverage information for all coverage group
// types." 19.11 (L33762-33764): "the get_inst_coverage() method returns the
// coverage of the specific coverage instance on which it is invoked." The
// text is silent on how loaded information combines with live instances.
module tb;
  bit [1:0] a;
  covergroup g;
    option.name = "g1";
    coverpoint a { bins lo = {0}; bins hi = {1}; }
  endgroup
  g g1 = new;
  initial begin
    if ($test$plusargs("load")) begin
      $load_coverage_db("s46_d2.db");
      $display("loaded %0.2f %0.2f", g::get_coverage(), g1.get_inst_coverage());
      a = 1; g1.sample();
      $display("joined %0.2f %0.2f", g::get_coverage(), g1.get_inst_coverage());
      $display("total %0.2f", $get_coverage());
    end else begin
      a = 0; g1.sample();
      $display("first %0.2f %0.2f", g::get_coverage(), g1.get_inst_coverage());
      $set_coverage_db_name("s46_d2.db");
    end
    $finish;
  end
endmodule
