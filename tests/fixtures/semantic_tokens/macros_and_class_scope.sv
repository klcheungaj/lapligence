// Semantic-token requirements ST-60 to ST-66 (docs/semantic_tokens.md):
// macro names, directive operands, macro formal arguments and bodies,
// class-scope names and delays.
`define A 32
`define ABC(x,y) ((x)+(y))
`define WR(v) Util2#($size(v))::write_dr(v)
`define COUNT cnt = cnt + 1; if (cnt > `A) cnt = 0
`define TWICE(e) `ABC(e, e)
`define SAME_KIND hits
`define MIXED_KIND mixed
`define NEVER_USED never_seen + 1
`define STR "body text"
`default_nettype none
`ifdef XXX
`elsif ZZZ
`endif
`ifndef YYY
`endif
`undef N
virtual class Util2 #(parameter int W = 1);
  typedef logic [W-1:0] T;
  static task write_dr(input logic [W-1:0] d);
    $display("%0d", d);
  endtask
  static function int width();
    return W;
  endfunction
  function int inst_width();
    return W;
  endfunction
endclass
class C;
  class Inner;
    static function int depth();
      return 2;
    endfunction
  endclass
  static function int f();
    return 1;
  endfunction
endclass
module mac_top(input logic clk);
  logic [3:0] p, q, in;
  logic [7:0] w;
  int cnt;
  int hits;
  int mixed;
  Util2#(4)::T tv;
  Util2#(8)::T tw;
  assign w = `A + `ABC(p, q);
  initial begin
    Util2#($size(in))::write_dr(in);
    `WR(in);
    cnt = C::f() + Util2#(2)::width() + C::Inner::depth();
    `COUNT;
    cnt = `TWICE(cnt);
    `SAME_KIND = 1;
    `MIXED_KIND = 1;
    $display(`STR);
    #10 cnt = 1;
    #1ns cnt = 2;
  end
  initial begin : blk
    localparam int BL = 4;
    cnt = Util2#(BL)::width();
  end
endmodule
module mac_other;
  int hits;
  localparam int mixed = 3;
  initial begin
    `SAME_KIND = 2;
    $display("%0d", `MIXED_KIND);
  end
endmodule
`default_nettype wire
