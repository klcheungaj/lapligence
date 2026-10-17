// Gate-fixes-2 item 1: always_comb does not follow the implementation a
// virtual-interface method call selects at run time.
interface bus_if;
  logic [7:0] a;
  function automatic logic [7:0] get();
    return a;
  endfunction
endinterface
module tb;
  bus_if x();
  virtual bus_if v = x;
  logic [7:0] y;
  always_comb y = v.get();
  initial begin
    #1 x.a = 8'h03;
    #1 $display("%h", y);
    $finish;
  end
endmodule
