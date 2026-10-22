// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_044/bins.sv
// SIM-044: explicit value bins resolved against the coverpoint type (the 19.5.6 examples), enum and signed automatic bins, and iff guards (IEEE 1800-2009 19.5, 19.5.1, 19.5.2, 19.5.6, 19.11.1).
module tb;
  bit [2:0] p1;
  bit signed [2:0] p2;
  typedef enum {RED, GREEN = 5, BLUE} color_t;
  color_t color;
  bit en;
  covergroup g1; option.per_instance = 1;
    c1: coverpoint p1 {
      bins b1 = {1, [2:5], [6:10]};
      bins b2 = {-1, [1:10], 15};
    }
    c2: coverpoint p2 {
      bins b3 = {1, [2:5], [6:10]};
      bins b4 = {-1, [1:10], 15};
    }
  endgroup
  covergroup g2; option.per_instance = 1;
    ce: coverpoint color;
    cs: coverpoint p2 { option.auto_bin_max = 3; }
    ci: coverpoint p1 iff (en);
    cu: coverpoint p1 { bins only_seven = {7}; bins never = {15}; }
  endgroup
  g1 a = new;
  g2 b = new;
  initial begin
    p1 = 5;
    p2 = 0;
    a.sample();
    $display("p1=5 c1=%0.2f p2=0 c2=%0.2f", a.c1.get_inst_coverage(), a.c2.get_inst_coverage());
    p1 = 0;
    p2 = -1;
    a.sample();
    $display("p1=0 c1=%0.2f p2=-1 c2=%0.2f", a.c1.get_inst_coverage(), a.c2.get_inst_coverage());
    color = BLUE;
    p2 = -4;
    en = 0;
    p1 = 7;
    b.sample();
    p2 = -1;
    en = 1;
    p1 = 1;
    b.sample();
    $display("enum=%0.2f signed=%0.2f iff=%0.2f values=%0.2f", b.ce.get_inst_coverage(),
             b.cs.get_inst_coverage(), b.ci.get_inst_coverage(), b.cu.get_inst_coverage());
    $finish;
  end
endmodule
