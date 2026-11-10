`timescale 1ns/1ns
// `@(vif.cb)` waits on the clocking block event of the instance the handle
// is bound to when the wait arms (SV 14.13, 25.9): through a handle, a
// modport view and a class property, under `iff`, in an event list and in
// parallel waits.
interface bus(input bit clk);
  logic [7:0] data;
  clocking cb @(posedge clk);
    input data;
  endclocking
  clocking cn @(negedge clk);
    input data;
  endclocking
  modport tb_mp(clocking cb);
endinterface

class driver;
  virtual bus vif;
  function new(virtual bus v); vif = v; endfunction
  task run();
    @(vif.cb);
    $display("%0t drv %h", $time, vif.cb.data);
  endtask
endclass

module tb;
  bit clk = 1'b0;
  always #5 clk = ~clk;
  bus b1(clk);
  bus b2(clk);
  virtual bus v;
  virtual bus.tb_mp vp;
  bit en = 0;
  driver d;
  initial begin
    b1.data = 8'h11;
    b2.data = 8'h22;
    v = b1;
    vp = b2.tb_mp;
    d = new(b2);
    #12 en = 1'b1;
    #9 b1.data = 8'h33;
  end
  initial begin
    @(v.cn) $display("%0t cn %h", $time, v.cn.data);
    @(v.cb iff en) $display("%0t iff %h", $time, v.cb.data);
    @(vp.cb or posedge en) $display("%0t or %h", $time, vp.cb.data);
    fork
      d.run();
      begin repeat (2) @(v.cb); $display("%0t fork %h", $time, v.cb.data); end
    join
    $finish;
  end
endmodule
