// A nested interface instance selected through a virtual interface whose
// type has no instance in the design: the handle can only be null, so the
// selection is a run-time error when it executes (SV 25.9).
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
  leaf_if spare();
  initial begin
    user u = new;
    $display("before");
    u.lv = u.o.l;
    $display("after");
    $finish;
  end
endmodule
