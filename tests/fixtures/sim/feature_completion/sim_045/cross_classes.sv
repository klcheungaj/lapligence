// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/cross_classes.sv
// SIM-045: the selects of `abcd` split each crossed coverpoint into 33 value
// classes (32 single bins and the rest); 33^4 combinations exceed the 2^20
// llg counts, so constructing the instance reports a run-time error, the run
// goes on and the cross counts no bins (llg limitation). The coverpoints
// still count: 1 of 64 bins.
module tb;
  bit [7:0] a, b, c, d;
  covergroup g;
    option.per_instance = 1;
    ca: coverpoint a { bins v[] = {[0:63]}; }
    cb: coverpoint b { bins v[] = {[0:63]}; }
    cc: coverpoint c { bins v[] = {[0:63]}; }
    cd: coverpoint d { bins v[] = {[0:63]}; }
    abcd: cross ca, cb, cc, cd {
      bins sa = (
        binsof(ca) intersect {0} || binsof(ca) intersect {1} || binsof(ca) intersect {2} ||
        binsof(ca) intersect {3} || binsof(ca) intersect {4} || binsof(ca) intersect {5} ||
        binsof(ca) intersect {6} || binsof(ca) intersect {7} || binsof(ca) intersect {8} ||
        binsof(ca) intersect {9} || binsof(ca) intersect {10} || binsof(ca) intersect {11} ||
        binsof(ca) intersect {12} || binsof(ca) intersect {13} || binsof(ca) intersect {14} ||
        binsof(ca) intersect {15} || binsof(ca) intersect {16} || binsof(ca) intersect {17} ||
        binsof(ca) intersect {18} || binsof(ca) intersect {19} || binsof(ca) intersect {20} ||
        binsof(ca) intersect {21} || binsof(ca) intersect {22} || binsof(ca) intersect {23} ||
        binsof(ca) intersect {24} || binsof(ca) intersect {25} || binsof(ca) intersect {26} ||
        binsof(ca) intersect {27} || binsof(ca) intersect {28} || binsof(ca) intersect {29} ||
        binsof(ca) intersect {30} || binsof(ca) intersect {31});
      bins sb = (
        binsof(cb) intersect {0} || binsof(cb) intersect {1} || binsof(cb) intersect {2} ||
        binsof(cb) intersect {3} || binsof(cb) intersect {4} || binsof(cb) intersect {5} ||
        binsof(cb) intersect {6} || binsof(cb) intersect {7} || binsof(cb) intersect {8} ||
        binsof(cb) intersect {9} || binsof(cb) intersect {10} || binsof(cb) intersect {11} ||
        binsof(cb) intersect {12} || binsof(cb) intersect {13} || binsof(cb) intersect {14} ||
        binsof(cb) intersect {15} || binsof(cb) intersect {16} || binsof(cb) intersect {17} ||
        binsof(cb) intersect {18} || binsof(cb) intersect {19} || binsof(cb) intersect {20} ||
        binsof(cb) intersect {21} || binsof(cb) intersect {22} || binsof(cb) intersect {23} ||
        binsof(cb) intersect {24} || binsof(cb) intersect {25} || binsof(cb) intersect {26} ||
        binsof(cb) intersect {27} || binsof(cb) intersect {28} || binsof(cb) intersect {29} ||
        binsof(cb) intersect {30} || binsof(cb) intersect {31});
      bins sc = (
        binsof(cc) intersect {0} || binsof(cc) intersect {1} || binsof(cc) intersect {2} ||
        binsof(cc) intersect {3} || binsof(cc) intersect {4} || binsof(cc) intersect {5} ||
        binsof(cc) intersect {6} || binsof(cc) intersect {7} || binsof(cc) intersect {8} ||
        binsof(cc) intersect {9} || binsof(cc) intersect {10} || binsof(cc) intersect {11} ||
        binsof(cc) intersect {12} || binsof(cc) intersect {13} || binsof(cc) intersect {14} ||
        binsof(cc) intersect {15} || binsof(cc) intersect {16} || binsof(cc) intersect {17} ||
        binsof(cc) intersect {18} || binsof(cc) intersect {19} || binsof(cc) intersect {20} ||
        binsof(cc) intersect {21} || binsof(cc) intersect {22} || binsof(cc) intersect {23} ||
        binsof(cc) intersect {24} || binsof(cc) intersect {25} || binsof(cc) intersect {26} ||
        binsof(cc) intersect {27} || binsof(cc) intersect {28} || binsof(cc) intersect {29} ||
        binsof(cc) intersect {30} || binsof(cc) intersect {31});
      bins sd = (
        binsof(cd) intersect {0} || binsof(cd) intersect {1} || binsof(cd) intersect {2} ||
        binsof(cd) intersect {3} || binsof(cd) intersect {4} || binsof(cd) intersect {5} ||
        binsof(cd) intersect {6} || binsof(cd) intersect {7} || binsof(cd) intersect {8} ||
        binsof(cd) intersect {9} || binsof(cd) intersect {10} || binsof(cd) intersect {11} ||
        binsof(cd) intersect {12} || binsof(cd) intersect {13} || binsof(cd) intersect {14} ||
        binsof(cd) intersect {15} || binsof(cd) intersect {16} || binsof(cd) intersect {17} ||
        binsof(cd) intersect {18} || binsof(cd) intersect {19} || binsof(cd) intersect {20} ||
        binsof(cd) intersect {21} || binsof(cd) intersect {22} || binsof(cd) intersect {23} ||
        binsof(cd) intersect {24} || binsof(cd) intersect {25} || binsof(cd) intersect {26} ||
        binsof(cd) intersect {27} || binsof(cd) intersect {28} || binsof(cd) intersect {29} ||
        binsof(cd) intersect {30} || binsof(cd) intersect {31});
    }
  endgroup
  g cg;
  initial begin
    cg = new;
    a = 8'd1; b = 8'd2; c = 8'd3; d = 8'd4;
    cg.sample();
    $display("ca %0.2f abcd %0.2f", cg.ca.get_inst_coverage(), cg.abcd.get_inst_coverage());
    $finish;
  end
endmodule
