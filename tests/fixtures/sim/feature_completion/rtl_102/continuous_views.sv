// IEEE 1800-2009 25.5.4: a modport expression port `.p(expr)` reads and
// writes its port expression in the connected interface instance.
interface bus_if #(parameter int W = 4);
  logic [2*W-1:0] data;
  wire [7:0] bus;
  logic [1:0][W-1:0] lane;
  modport lo (input .nib(data[W-1:0]), input .l1(lane[1]), input .top(bus[7:4]),
              output .bot(bus[3:0]));
  modport hi (output .hi(data[2*W-1:W]), output .l0(lane[0]), output .top(bus[7:4]));
endinterface
module pass_in(input logic [3:0] i, output logic [3:0] o);
  assign o = i;
endmodule
module pass_out(output logic [3:0] o, input logic [3:0] i);
  assign o = i;
endmodule
module rd(bus_if.lo b, input logic [3:0] u, output logic [3:0] y, output logic [3:0] y2,
          output logic [3:0] t);
  pass_in pi(.i(b.nib), .o(y));
  assign y2 = b.l1;
  assign t = b.top;
  assign b.bot = u;
endmodule
module wr(bus_if.hi b, input logic [3:0] v);
  pass_out po(.o(b.hi), .i(v));
  assign b.l0 = ~v;
  assign b.top = v ^ 4'hf;
endmodule
// Forwarding the interface port through an intermediate module.
module mid(bus_if.hi b, input logic [3:0] v);
  wr w(b, v);
endmodule
module tb;
  bus_if #(.W(4)) bi[2]();
  logic [3:0] y, y2, t, u, v;
  rd r(bi[1], u, y, y2, t);
  mid m(bi[1], v);
  for (genvar g = 0; g < 1; g++) begin : gen
    logic [3:0] gy, gy2, gt;
    rd r0(bi[0], 4'h6, gy, gy2, gt);
    mid m0(bi[0], 4'h1);
  end
  initial begin
    bi[1].data[3:0] = 4'h7; bi[1].lane[1] = 4'h3; v = 4'h9; u = 4'h5;
    bi[0].data[3:0] = 4'h2; bi[0].lane[1] = 4'hc;
    #1 $display("%h %h %h %h %h %h %h", y, y2, t, bi[1].data, bi[1].lane[0], bi[1].bus, r.b.nib);
    $display("%h %h %h %h %h", gen[0].gy, gen[0].gy2, gen[0].gt, bi[0].data, bi[0].bus);
    bi[1].data[3:0] = 4'he; v = 4'h0; u = 4'ha;
    #1 $display("%h %h %h %h %h %h", y, y2, t, bi[1].data, bi[1].lane[0], bi[1].bus);
    $finish(0);
  end
endmodule
