// Selecting a nested interface instance through a null virtual interface is
// a run-time error at the selection (SV 25.9).
`timescale 1ns/1ns
interface leaf_if;
  logic v;
endinterface
interface outer_if;
  leaf_if l();
endinterface
class user;
  virtual outer_if o;
  virtual leaf_if lv;
endclass
module tb;
  outer_if bound();
  initial begin
    user u = new;
    $display("before");
    u.lv = u.o.l;
    $display("after");
    $finish;
  end
endmodule
