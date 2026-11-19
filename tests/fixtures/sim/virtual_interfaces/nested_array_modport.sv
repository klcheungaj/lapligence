// Elements of an instance array declared in an interface, and a modport view
// of a nested instance, selected through a virtual interface (SV 25.9); a
// wait on a member of a nested instance through the handle.
`timescale 1ns/1ns
interface leaf_if;
  logic [3:0] v;
  logic [3:0] w;
  modport drv(output v, input w);
endinterface
interface outer_if;
  leaf_if ls[3]();
  leaf_if up[1:2]();
  leaf_if one();
endinterface
class user;
  virtual outer_if o;
  virtual leaf_if lv;
  virtual leaf_if.drv dv;
  function void go();
    lv = o.ls[0];
    lv.v = 1;
    lv = o.ls[2];
    lv.v = 3;
    o.ls[1].v = 2;
    lv = o.up[2];
    lv.v = 4'ha;
    o.up[1].v = 4'hb;
    dv = o.one;
    dv.v = 4'h5;
  endfunction
endclass
module tb;
  outer_if o();
  initial begin
    user u = new;
    u.o = o;
    o.one.w = 4'h9;
    u.go();
    #1 $display("%0d %0d %0d %h %h %h %h", o.ls[0].v, o.ls[1].v, o.ls[2].v,
                o.up[2].v, o.up[1].v, o.one.v, u.dv.w);
    fork
      #3 o.one.w = 4'h6;
      begin
        @(u.o.one.w);
        $display("%0t w=%h", $time, u.o.one.w);
      end
    join
    $finish;
  end
endmodule
