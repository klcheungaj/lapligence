`timescale 1ns/1ns
interface ifc; event ie; endinterface
module tb;
  ifc i0(), i1();
  virtual ifc vif;
  virtual ifc va[2];
  class drv;
    virtual ifc v;
    function void fire();
      -> v.ie;
    endfunction
  endclass
  drv dr = new;
  int k = 1;
  always @(i0.ie) $display("%0t i0.ie", $time);
  always @(i1.ie) $display("%0t i1.ie", $time);
  initial begin
    vif = i0; va[0] = i0; va[1] = i1; dr.v = i1;
    #1 -> vif.ie;
    #1 ->> vif.ie;
    #1 ->> #2 va[k].ie;
    k = 0;
    #5 dr.fire();
    #1 -> va[k].ie;
    #1 $display("%b", vif.ie.triggered);
    -> vif.ie; $display("%b", vif.ie.triggered);
    fork begin @(vif.ie); $display("%0t vwait", $time); end join_none
    #1 -> vif.ie;
    #1 vif = i1; -> vif.ie; 
    #1 $finish(0);
  end
endmodule
