// Gate item G1: only `=` and `<=` dispatch a write to an element of an
// unpacked array member on the instance a virtual interface is bound to.
interface bus_if;
  logic [7:0] mem [0:3];
endinterface
module tb;
  bus_if a();
  virtual bus_if v = a;
  initial begin
    v.mem[1] += 8'd2;
    $finish;
  end
endmodule
