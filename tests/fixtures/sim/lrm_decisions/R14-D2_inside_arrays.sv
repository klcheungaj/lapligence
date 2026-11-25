// IEEE 1800-2009 11.4.13: "If an expression in the list is an unpacked array,
// its elements are traversed by descending into the array until reaching a
// singular value."
//
// R14-D2: arrays of integral elements of every kind are traversed, variables
// and parameters alike, also in a constant expression. (llg rejects arrays of
// strings; see docs/lrm_decisions.md.)
`timescale 1ns/1ns
module tb;
  parameter int PF[2] = '{5, 9};
  parameter int PM[2][2] = '{'{1, 2}, '{3, 4}};
  parameter logic [3:0] PX[2] = '{4'b1x00, 4'b0011};
  parameter bit IN = 9 inside {PF}, OUT = 8 inside {PF};
  int f[2] = '{5, 9};
  int m[2][2] = '{'{1, 2}, '{3, 4}};
  logic [3:0] x[2] = '{4'b1x00, 4'b0011};
  int d[], q[$], a[string];
  int v = 9;
  initial begin
    d = new[2];
    d[1] = 9;
    q.push_back(1);
    q.push_back(9);
    a["z"] = 9;
    $display("fixed %b %b", v inside {f}, 8 inside {f});
    $display("dimensions %b %b", 3 inside {m}, 8 inside {m});
    $display("wildcard %b %b", 4'b1100 inside {x}, 4'b0000 inside {x});
    $display("dynamic %b %b", v inside {d}, 8 inside {d});
    $display("queue %b %b", v inside {q}, 8 inside {q});
    $display("associative %b %b", v inside {a}, 8 inside {a});
    $display("mixed %b %b", v inside {[1:4], f}, 6 inside {[1:4], f, 6});
    $display("parameter %b %b", v inside {PF}, 8 inside {PF});
    $display("parameter dimensions %b %b", 3 inside {PM}, 8 inside {PM});
    $display("parameter wildcard %b %b", 4'b1100 inside {PX}, 4'b0000 inside {PX});
    $display("constant %b %b", IN, OUT);
    $finish(0);
  end
endmodule
