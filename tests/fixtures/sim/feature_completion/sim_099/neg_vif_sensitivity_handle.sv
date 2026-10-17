// Gate-fixes-2 item 1: a member read through an element of a
// virtual-interface array is a handle the implicit sensitivity cannot
// evaluate again when the process arms.
interface bus_if;
  logic [7:0] a;
endinterface
module tb;
  bus_if x();
  virtual bus_if va [2] = '{x, x};
  logic [7:0] y;
  always_comb y = va[1].a;
  initial begin
    #1 x.a = 8'h03;
    #1 $display("%h", y);
    $finish;
  end
endmodule
