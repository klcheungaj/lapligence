`timescale 1ns/1ns
// SIM-033 A02: expression-backed and real clocking inputs declared in an
// interface, read through a concrete instance, a modport clocking port of a
// program, a virtual interface that is rebound, and a virtual modport view.
interface bus (input logic clk);
  logic [7:0] data;
  real level;
  clocking cb @(posedge clk);
    input data;
    input lo = data[3:0];
    input swap = {data[3:0], data[7:4]};
    input level;
    input #0 scaled = level * 4.0;
    input #2 old = level;
  endclocking
  modport tb_mp (clocking cb);
endinterface

program reader (bus.tb_mp p);
  initial begin
    repeat (3) begin
      @(p.cb);
      $display("prog %0t %h %h %h %.2f %.2f %.2f", $time, p.cb.data, p.cb.lo,
               p.cb.swap, p.cb.level, p.cb.scaled, p.cb.old);
    end
    #10;
  end
endprogram

module tb;
  logic clk = 1'b0;
  bus b (clk);
  bus b2 (clk);
  reader r (b.tb_mp);
  virtual bus vb;
  virtual bus.tb_mp vp;

  always #5 clk = ~clk;

  initial begin
    b.data = 8'h3C;
    b.level = 1.5;
    b2.data = 8'h55;
    b2.level = 9.5;
    #12 b.data = 8'hA7;
    b.level = 2.5;
    @(posedge clk) b.level = 7.0;
    #6 b.data = 8'h01;
    b.level = 0.25;
  end

  initial begin
    vb = b;
    vp = b;
    #6 $display("mod %0t %h %h %h %.2f %.2f %.2f", $time, b.cb.data, b.cb.lo,
                b.cb.swap, b.cb.level, b.cb.scaled, b.cb.old);
    #10 $display("vif %0t %h %h %h %.2f %.2f %.2f", $time, vb.cb.data, vb.cb.lo,
                 vb.cb.swap, vb.cb.level, vb.cb.scaled, vb.cb.old);
    vb = b2;
    #10 $display("vif %0t %h %h %h %.2f %.2f %.2f", $time, vb.cb.data, vb.cb.lo,
                 vb.cb.swap, vb.cb.level, vb.cb.scaled, vb.cb.old);
    $display("view %0t %h %h %h %.2f %.2f %.2f", $time, vp.cb.data, vp.cb.lo,
             vp.cb.swap, vp.cb.level, vp.cb.scaled, vp.cb.old);
  end
endmodule
