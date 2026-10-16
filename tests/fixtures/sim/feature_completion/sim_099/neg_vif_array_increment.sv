// Gate item G1: an increment of an element of an unpacked array member
// through a virtual interface is not dispatched on the bound instance.
interface bus_if;
  logic [7:0] mem [0:3];
endinterface
module tb;
  bus_if a();
  virtual bus_if v = a;
  initial begin
    v.mem[1]++;
    $finish;
  end
endmodule
