// SV2009 6.20, 6.21, 7.2, 26.2-26.3, 3.12.1: package and compilation-unit
// parameters of typedef, packed-structure, unpacked-structure and unpacked-
// array types are constants available to module declaration initializers and
// procedural code, independently of the order in which scopes are collected.
// A `const` variable is a runtime constant initialized once (its initializer
// may call a function and read variables); it is not a constant expression.
localparam int UNIT_I = 5;
typedef struct packed { logic [3:0] a; logic [3:0] b; } unit_pair_t;
localparam unit_pair_t UNIT_P = '{4'h1, 4'h2};
typedef struct { int x; int y; } urec_t;
localparam urec_t UNIT_R = '{3, 4};

package pk;
  typedef logic [7:0] byte_t;
  localparam byte_t PB = 8'h21;
  typedef struct packed { logic [3:0] a; logic [3:0] b; } pair_t;
  localparam pair_t PP = '{4'h3, 4'h4};
  parameter int PI = 9;
  typedef struct { int x; int y; } rec_t;
  localparam rec_t PR = '{7, 8};
  localparam int ARR [0:3] = '{10, 20, 30, 40};
  localparam rec_t PR2 = '{PR.y, ARR[2]};
  const int PC = PI - 4;
  int pa = PC + 1;
endpackage

module tb;
  import pk::*;
  unit_pair_t up = UNIT_P;
  pair_t pp = PP;
  byte_t pb = PB;
  int i = UNIT_I + PI;
  urec_t ur = UNIT_R;
  rec_t pr = PR;
  int s = ARR[1] + PR2.x + PR2.y;
  int idx = 2;
  int seed = 6;
  function int calc(int v);
    return v * 7;
  endfunction
  const int c = calc(seed);
  localparam int L = PI * 2;
  initial begin
    $display("%h %h %h %0d %0d %0d %0d %0d %0d", up, pp, pb, i, ur.x, ur.y, pr.x, pr.y, s);
    $display("%h %0d %0d %0d", UNIT_P.b, UNIT_R.y, pk::PR.x, ARR[idx]);
    $display("%0d %0d %0d %0d", PC, pa, c, L);
    $finish(0);
  end
endmodule
