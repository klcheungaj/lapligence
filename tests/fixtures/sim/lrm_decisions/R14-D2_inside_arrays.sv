// IEEE 1800-2009 11.4.13: "If an expression in the list is an unpacked array,
// its elements are traversed by descending into the array until reaching a
// singular value."
//
// R14-D2: arrays of every kind are traversed, whether their elements are
// integral or strings, variables, constants and parameters alike, also in a
// constant expression.
`timescale 1ns/1ns
module tb;
  parameter int PF[2] = '{5, 9};
  parameter int PM[2][2] = '{'{1, 2}, '{3, 4}};
  parameter logic [3:0] PX[2] = '{4'b1x00, 4'b0011};
  parameter bit IN = 9 inside {PF}, OUT = 8 inside {PF};
  parameter string PS[2] = '{"k0", "k1"};
  parameter string PT[2][2] = '{'{"a", "b"}, '{"c", "d"}};
  parameter bit SIN = "k1" inside {PS}, SOUT = "zz" inside {PS}, TIN = "c" inside {PT};
  int f[2] = '{5, 9};
  int m[2][2] = '{'{1, 2}, '{3, 4}};
  logic [3:0] x[2] = '{4'b1x00, 4'b0011};
  int d[], q[$], a[string];
  string sf[2] = '{"f0", "f1"};
  string sm[2][3] = '{'{"a", "b", "c"}, '{"d", "e", "f"}};
  const string sc[2] = '{"c0", "c1"};
  string sd[], sq[$], sa[string], si[int];
  int v = 9;
  string s = "f1";
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
    sd = new[2];
    sd[1] = "d1";
    sq.push_back("q0");
    sa["k"] = "v";
    si[5] = "i5";
    $display("string fixed %b %b %b", s inside {sf}, "f0" inside {sf}, "zz" inside {sf});
    $display("string dimensions %b %b", "e" inside {sm}, "g" inside {sm});
    $display("string row %b %b", "e" inside {sm[1]}, "a" inside {sm[1]});
    $display("string const %b %b", "c1" inside {sc}, "zz" inside {sc});
    $display("string dynamic %b %b", "d1" inside {sd}, "zz" inside {sd});
    $display("string queue %b %b", "q0" inside {sq}, "zz" inside {sq});
    $display("string associative %b %b %b", "v" inside {sa}, "k" inside {sa}, "i5" inside {si});
    $display("string parameter %b %b", "k1" inside {PS}, "zz" inside {PS});
    $display("string parameter dimensions %b %b", "c" inside {PT}, "c" inside {PT[0]});
    $display("string constant %b %b %b", SIN, SOUT, TIN);
    $display("string mixed %b %b", s inside {"x", sq, sf}, "fz" inside {sq, [sf[0]:"fzz"]});
    case (s) inside
      sq: $display("case queue");
      sf: $display("case fixed");
      default: $display("case none");
    endcase
    $finish(0);
  end
endmodule
