// Element ranges as continuous targets, port actuals, alias and force
// targets and process sensitivity (IEEE 1800-2009 7.4.5, 10.3, 10.6.2,
// 10.11, 23.3.3).
module invert(input logic [15:0] i, output logic [15:0] o);
  assign o = ~i;
endmodule
module tb;
  logic [3:0][7:0] w;
  logic [3:0][7:0] v;
  wire [3:0][7:0] nw;
  wire [15:0] mid;
  wire [3:0][7:0] na [0:1];
  wire [15:0] slot;
  logic [1:0][7:0] src;
  logic [15:0] mon;
  logic [7:0] pick;
  integer i;
  alias mid = nw[2:1];
  assign nw[3:2] = src;
  assign nw[1:0] = w[1:0];
  assign na[1] = w;
  assign slot = na[1][3:2];
  invert u(.i(w[3:2]), .o(v[1:0]));
  assign v[3:2] = w[1:0];
  always_comb mon = w[2:1];
  always_comb pick = w[i];
  initial begin
    w = 32'h44332211;
    src = 16'h6655;
    i = 3;
    #1 $display("init v=%h nw=%h mid=%h slot=%h mon=%h pick=%h", v, nw, mid, slot, mon, pick);
    w[2:1] = 16'h7788;
    i = 1;
    #1 $display("update v=%h nw=%h mid=%h slot=%h mon=%h pick=%h", v, nw, mid, slot, mon, pick);
    force nw[2:1] = 16'hf00d;
    #1 $display("force nw=%h mid=%h", nw, mid);
    release nw[2:1];
    #1 $display("release nw=%h mid=%h", nw, mid);
    $finish(0);
  end
endmodule
