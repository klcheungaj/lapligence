// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_046/options.sv
// SIM-046: procedural option reads and writes through instances, coverpoints,
// crosses and the type name, set_inst_name, and the two-argument coverage
// queries (IEEE 1800-2009 19.7, 19.7.1, 19.8, 19.10, 19.11). Expected values
// are derived in readme.md.
module tb;
  bit [3:0] v;
  bit [1:0] w;
  covergroup g;
    option.comment = "g instance";
    type_option.comment = "g type";
    type_option.goal = 90;
    p: coverpoint v {
      bins lo = {[0:3]};
      bins mid = {[4:7]};
      bins hi = {[8:15]};
      type_option.weight = 2;
      option.comment = "p";
    }
    q: coverpoint w {
      bins z = {0};
      bins o = {1};
    }
    x: cross p, q;
  endgroup
  g a = new;
  g b = new;
  int c, t;
  real r;

  initial begin
    $display("names %s %s", a.option.name, b.option.name);
    $display("comments [%s] [%s] [%s] [%s]", a.option.comment, a.p.option.comment,
             a.q.option.comment, g::type_option.comment);
    $display("type %0d %0d %0d %0d %0d", g::type_option.weight, g::type_option.goal,
             g::p::type_option.weight, g::q::type_option.weight, g::type_option.merge_instances);
    $display("defaults %0d %0d %0d %0d %0d %0d %0d %0d", a.option.weight, a.option.goal,
             a.option.at_least, a.option.auto_bin_max, a.option.per_instance,
             a.option.get_inst_coverage, a.x.option.cross_num_print_missing,
             a.option.detect_overlap);
    a.option.weight = 3;
    a.p.option.at_least = 2;
    a.x.option.cross_num_print_missing = 5;
    b.option.goal = 50;
    g::q::type_option.goal = 75;
    $display("written %0d %0d %0d %0d %0d %0d", a.option.weight, a.p.option.at_least,
             a.q.option.at_least, a.x.option.cross_num_print_missing, b.option.goal,
             g::q::type_option.goal);
    v = 4'd1; w = 2'd0; a.sample();
    v = 4'd2; w = 2'd0; a.sample();
    v = 4'd5; w = 2'd1; a.sample();
    v = 4'd9; w = 2'd1; b.sample();
    r = a.get_inst_coverage(c, t);
    $display("a %0d %0d %f", c, t, r);
    r = a.p.get_inst_coverage(c, t);
    $display("a.p %0d %0d %f", c, t, r);
    r = a.x.get_inst_coverage(c, t);
    $display("a.x %0d %0d %f", c, t, r);
    r = b.get_inst_coverage(c, t);
    $display("b %0d %0d %f", c, t, r);
    r = g::get_coverage(c, t);
    $display("type %0d %0d %f", c, t, r);
    r = a.p.get_coverage(c, t);
    $display("type.p %0d %0d %f", c, t, r);
    g::type_option.merge_instances = 1;
    r = g::get_coverage(c, t);
    $display("merged %0d %0d %f", c, t, r);
    r = a.get_inst_coverage(c, t);
    $display("merged a %0d %0d %f", c, t, r);
    $display("merged total %f %f", $get_coverage(), a.get_coverage());
    g::type_option.merge_instances = 0;
    $display("unmerged %f", g::get_coverage());
    a.set_inst_name("first");
    b.option.name = "second";
    a.option.comment = "changed";
    g::p::type_option.comment = "p type";
    $display("renamed %s %s [%s] [%s]", a.option.name, b.option.name, a.option.comment,
             g::p::type_option.comment);
    $finish;
  end
endmodule
