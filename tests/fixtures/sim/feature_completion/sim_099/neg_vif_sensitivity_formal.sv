// Gate-fixes-2 item 1: always_comb follows the functions it calls (IEEE
// 1800-2009 9.2.2.2.1), but a member read through a function's handle formal
// has no handle value when the process arms its wait.
interface bus_if;
  logic [7:0] a;
endinterface
module tb;
  bus_if x();
  virtual bus_if v = x;
  logic [7:0] y;
  function automatic logic [7:0] read_a(virtual bus_if b);
    return b.a;
  endfunction
  always_comb y = read_a(v);
  initial begin
    #1 x.a = 8'h03;
    #1 $display("%h", y);
    $finish;
  end
endmodule
