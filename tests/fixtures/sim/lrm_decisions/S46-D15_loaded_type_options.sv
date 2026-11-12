// Decision S46-D15: a coverage database saves the type options in effect,
// but $load_coverage_db applies none of them: the loading run keeps its own
// type options (declared or written) and only adds the saved counts
// (S46-D2).
//
// Two runs in one directory: the first (no plusargs) samples gi = 0 and
// hi = 3, writes g's type_option.weight = 3 and saves s46_d15.db at the
// end; the second (+load) loads it. Expected: first run "first 3 1 62.50"
// ((50.00 x 3 + 100.00 x 1) / 4); second run "declared 1 1", then
// "loaded 1 1 75.00": the weights stay 1 and the loaded counts give g 50.00
// and h 100.00, so (50.00 + 100.00) / 2.
//
// IEEE 1800-2009 19.9 (SystemVerilog-1800-2009.txt L33683-33686):
// "$set_coverage_db_name ( filename ) sets the filename of the coverage
// database into which coverage information is saved at the end of a
// simulation run. $load_coverage_db ( filename ) loads from the given
// filename the cumulative coverage information for all coverage group
// types." 19.7.1 (L33402-33403): type options "describe particular
// features (or properties) of the covergroup type as a whole. They are
// analogous to static data members of classes." L33491-33492: "Other type
// options can be assigned procedurally at any time during simulation." The
// text is silent on whether loaded coverage information carries type
// options; this follows industry-standard behaviour.
module tb;
  bit [1:0] a;
  covergroup g;
    p: coverpoint a { bins lo = {0}; bins hi = {1}; }
  endgroup
  covergroup h;
    r: coverpoint a { bins all = {[0:3]}; }
  endgroup
  g gi = new;
  h hi = new;
  initial begin
    if ($test$plusargs("load")) begin
      $display("declared %0d %0d", gi.type_option.weight, hi.type_option.weight);
      $load_coverage_db("s46_d15.db");
      $display("loaded %0d %0d %0.2f", gi.type_option.weight, hi.type_option.weight,
               $get_coverage());
    end else begin
      a = 2'd0; gi.sample();
      a = 2'd3; hi.sample();
      gi.type_option.weight = 3;
      $display("first %0d %0d %0.2f", gi.type_option.weight, hi.type_option.weight,
               $get_coverage());
      $set_coverage_db_name("s46_d15.db");
    end
    $finish;
  end
endmodule
