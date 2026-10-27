// Semantic token requirements fixture (docs/semantic_tokens.md, ST rules).
`define REQ_W 4
`define REQ_ADD(a, b) ((a) + (b))
`ifdef REQ_UNDEFINED
wire req_inactive_net;
`endif
`timescale 1ns/1ps
`default_nettype wire
package req_pkg;
  typedef struct packed { logic [3:0] lo; logic hi; } pair_t;
  pair_t pv;
  typedef enum logic [1:0] { IDLE, RUN } state_t;
endpackage
interface req_if; logic s; endinterface
module req_child #(parameter int W = 1, parameter type T = logic)(input logic clk, output logic y, req_if b);
  assign y = clk;
endmodule
module req_top(input logic clk, input logic rst);
  import req_pkg::*;
  localparam int L = `REQ_W;
  typedef struct { int a; struct { int x; } inner; } s_t;
  typedef union packed { logic [1:0] u0; logic [1:0] u1; } u_t;
  s_t s;
  u_t un;
  state_t st;
  logic y, w, v;
  wire w1, w2;
  int q, r;
  logic z;
  event ev;
  string msg = "hi";
  req_if bi();
  req_child #(.W(L), .T(logic [1:0])) u0 (.clk(v), .y, .b(bi));
  req_child #(L) u1 (v, z, bi);
  alias w1 = w2;
  assign w = `REQ_ADD(v, rst);
  class req_cls; int prop; function void set(); prop = 1; endfunction endclass
  function automatic int twice(ref int n); return n + n; endfunction
  property p_req; @(posedge clk iff rst) disable iff (rst) always v; endproperty
  always_ff @(posedge clk) begin : blk
    unique case (q) 0: q <= 1; default: q <= 2; endcase
    if (q inside {[1:2]}) q <= 0; else q <= 3;
    for (int k = 0; k < 2; k++) if (k == 1) break;
  end
  final $display(msg);
  initial begin
    s.a = 1;
    s.inner.x = 2;
    s = '{a: 1, inner: '{x: 3}};
    req_pkg::pv.lo = 4'h1;
    un.u1 = 2'b01;
    st = IDLE;
    r = twice(r);
    force v = 1'b0;
    release v;
    disable blk;
    -> ev;
  end
endmodule
module req_imports import req_ext_pkg::*; #(parameter int PW = EXT_W) (input ext_t ei, output logic [EXT_L-1:0] eo);
  import req_ext_pkg::EXT_L;
  import req_ext_pkg::ext_e;
  import req_ext_pkg::EXT_ON;
  import req_ext_pkg::ext_inc;
  localparam int PL = EXT_L + ext_inc(PW) + req_ext_pkg::EXT_W;
  ext_e e2 = EXT_ON;
  assign eo = ei[EXT_L-1:0] + PL;
endmodule
