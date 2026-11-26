`timescale 1ns/1ns
interface ifc;
  event ie;
  event ea[2];
endinterface

module tb;
  ifc i();
  ifc j();
  virtual ifc v, w;
  virtual ifc va[2];
  event e, f;
  event la[2];
  bit s;

  class holder;
    virtual ifc v;
    task trigger();
      -> v.ea[0];
    endtask
  endclass
  holder o = new;

  task automatic waits(event x, string name);
    @x;
    $display("%s %0t", name, $time);
  endtask
  task automatic rebind(ref event x);
    x = f;
  endtask

  initial begin
    v = i;
    w = j;
    va = '{i, j};
    o.v = j;
    fork
      begin @v.ea[1]; $display("ea1 %0t", $time); end
      begin @(i.ea[1]); $display("i.ea1 %0t", $time); end
      begin wait (v.ea[0].triggered); $display("wait ea0 %0t", $time); end
      begin @va[1].ea[0]; $display("va1 %0t", $time); end
      begin @(v.ea[0] or v.ea[1]); $display("or %0t", $time); end
      waits(w.ea[0], "arg");
    join_none
    #1 -> v.ea[1];
    #1 -> v.ea[0];
    #1 o.trigger();
    #1 $display("%b", v.ea[0].triggered);
    v.ie = e;
    $display("%b %b", i.ie == e, v.ie == e);
    fork
      waits(v.ie, "ie");
      waits(i.ie, "i.ie");
    join_none
    #1 -> e;
    #1 v.ea[0] = f;
    v.ea[1] = v.ie;
    $display("%b %b", i.ea[0] == f, i.ea[1] == e);
    fork begin @v.ea[s]; $display("eas %0t", $time); end join_none
    #1 -> f;
    #1 s = 1;
    e = v.ea[s];
    $display("%b", e == i.ea[1]);
    v.ea[s] = null;
    $display("%b %b", v.ea[1] == null, j.ea[1] != null);
    v.ie = null;
    $display("%b %b", v.ie == null, i.ie == null);
    la = '{w.ea[1], e};
    v.ea = la;
    $display("%b %b", i.ea[0] == j.ea[1], i.ea[1] == e);
    v.ie = s ? e : f;
    $display("%b", v.ie == e);
    rebind(v.ie);
    $display("%b", i.ie == f);
    fork begin @i.ea[1]; $display("nbt %0t", $time); end join_none
    ->> #2 v.ea[1];
    #3 fork
      wait_order (v.ea[0], w.ea[0]) $display("order"); else $display("no order");
    join_none
    #1 -> v.ea[0];
    #1 -> w.ea[0];
    #1 $finish;
  end
endmodule
