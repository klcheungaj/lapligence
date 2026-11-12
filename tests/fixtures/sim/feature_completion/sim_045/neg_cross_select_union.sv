// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_045/neg_cross_select_union.sv
// SIM-045 negative: a 1024-term product joined by `||` to one more term
// expands to 1025 terms, more than the 1024 llg allows (llg limitation).
module tb;
  bit [7:0] a, b;
  covergroup g;
    ca: coverpoint a { bins v[] = {[0:63]}; }
    cb: coverpoint b { bins v[] = {[0:63]}; }
    ab: cross ca, cb {
      bins big = ((
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
        binsof(ca) intersect {30} || binsof(ca) intersect {31})
        && (
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
        binsof(cb) intersect {30} || binsof(cb) intersect {31}))
        || binsof(ca) intersect {40};
    }
  endgroup
  g c = new;
  initial $finish;
endmodule
