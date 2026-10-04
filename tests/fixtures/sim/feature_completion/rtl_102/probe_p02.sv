// RTL-099 audit probe p02: a procedural write to data[3:0] and a continuous
// write through `.hi(data[7:4])` own disjoint longest static prefixes.
interface bus_if;
  logic [7:0] data;
  modport lo (input .nib(data[3:0]));
  modport wr (output .hi(data[7:4]));
endinterface
module rd(bus_if.lo b, output logic [3:0] y);
  assign y = b.nib;
endmodule
module wrm(bus_if.wr b, input logic [3:0] v);
  assign b.hi = v;
endmodule
module tb;
  bus_if bi();
  logic [3:0] y;
  logic [3:0] v;
  rd r(bi, y);
  wrm w(bi, v);
  initial begin
    bi.data[3:0] = 4'h5; v = 4'ha;
    #1 $display("%h %h", y, bi.data);
    $finish(0);
  end
endmodule
