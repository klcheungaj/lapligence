// Gate item G1: a whole unpacked array member through a virtual interface
// has no per-instance array value in llg; only its elements are reachable.
interface bus_if;
  logic [7:0] mem [0:3];
endinterface
module tb;
  bus_if a();
  virtual bus_if v = a;
  logic [7:0] copy [0:3];
  initial begin
    copy = v.mem;
    $finish;
  end
endmodule
