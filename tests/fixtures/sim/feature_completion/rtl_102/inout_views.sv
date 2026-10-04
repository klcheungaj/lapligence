// IEEE 1800-2009 25.5.4 with 6.6.1: an inout modport expression port over
// interface net bits is one more driver of those bits; drivers resolve.
interface bus_if;
  wire [3:0] pins;
  modport dev (inout .io(pins[1:0]), input .all_pins(pins));
endinterface
module dev(bus_if.dev b, input logic en, input logic [1:0] v, output logic [1:0] seen,
           output logic [3:0] all);
  assign b.io = en ? v : 2'bzz;
  assign seen = b.io;
  assign all = b.all_pins;
endmodule
module tb;
  bus_if bi();
  logic en;
  logic [1:0] v, seen;
  logic [3:0] all;
  dev d(bi, en, v, seen, all);
  assign bi.pins[0] = en ? 1'bz : 1'b1;
  assign bi.pins[3] = 1'b0;
  initial begin
    en = 1'b1; v = 2'b10;
    #1 $display("%b %b %b", seen, all, bi.pins);
    en = 1'b0;
    #1 $display("%b %b %b", seen, all, bi.pins);
    $finish(0);
  end
endmodule
