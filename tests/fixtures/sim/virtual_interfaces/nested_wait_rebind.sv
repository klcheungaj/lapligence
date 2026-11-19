// A wait on a member of a nested interface instance through a class's
// virtual interface (SV 25.9) wakes when the handle is rebound to another
// instance whose member holds another value, as a wait through a handle does.
`timescale 1ns/1ns
interface leaf_if;
  logic [3:0] w;
endinterface
interface outer_if;
  leaf_if one();
endinterface
class user;
  virtual outer_if o;
endclass
module tb;
  outer_if o1();
  outer_if o2();
  user u;
  initial begin
    u = new;
    u.o = o1;
    o1.one.w = 1;
    o2.one.w = 2;
    fork
      begin
        @(u.o.one.w);
        $display("%0t woke w=%h", $time, u.o.one.w);
      end
      begin
        #2 u.o = o2;
        #2 o1.one.w = 4'h7;
        #2 o2.one.w = 4'h9;
      end
    join_any
    #10 $finish;
  end
endmodule
