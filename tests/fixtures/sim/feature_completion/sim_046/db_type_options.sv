// SIM-046: the coverage database saves the type options in effect at the
// save (IEEE 1800-2009 19.7.1, 19.9; decision S46-D15), and
// $load_coverage_db adds the saved counts but applies none of them: the
// loading run keeps its own type options. The test runs the model three
// times in one directory: without plusargs it samples, writes type options
// and saves t.db; +load=t.db loads it; +load=old.db loads the same run
// rewritten in the format version 1, which holds no type options and loads
// the same way. Derivations: readme.md.
module tb;
  bit [1:0] a;
  covergroup g;
    type_option.weight = 2;
    type_option.comment = "g declared";
    p: coverpoint a { bins lo = {0}; bins hi = {1}; }
    q: coverpoint a { bins two = {2}; bins three = {3}; }
  endgroup
  covergroup h;
    r: coverpoint a { bins all = {[0:3]}; }
  endgroup
  g gi = new;
  h hi = new;
  string db;

  task automatic report(string label);
    $display("%s g weight %0d goal %0d merge %0d comment \"%s\"", label,
             g::type_option.weight, g::type_option.goal, g::type_option.merge_instances,
             g::type_option.comment);
    $display("%s g::p weight %0d goal %0d comment \"%s\"", label, g::p::type_option.weight,
             g::p::type_option.goal, g::p::type_option.comment);
    $display("%s h weight %0d", label, h::type_option.weight);
    $display("%s g %0.2f h %0.2f total %0.2f", label, g::get_coverage(), h::get_coverage(),
             $get_coverage());
  endtask

  initial begin
    if ($value$plusargs("load=%s", db)) begin
      report("declared");
      $load_coverage_db(db);
      report("loaded");
      g::type_option.weight = 5;
      report("written");
    end else begin
      a = 2'd0; gi.sample();
      a = 2'd3; hi.sample();
      g::type_option.weight = 3;
      g::type_option.goal = 90;
      g::type_option.merge_instances = 1;
      g::type_option.comment = "g saved";
      g::p::type_option.weight = 4;
      g::p::type_option.goal = 80;
      g::p::type_option.comment = "p saved";
      h::type_option.weight = 2;
      report("first");
      $set_coverage_db_name("t.db");
    end
    $finish;
  end
endmodule
