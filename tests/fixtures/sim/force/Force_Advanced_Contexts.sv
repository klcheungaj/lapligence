// IEEE 1800-2009 §10.6.2 and IEEE 1364-2001 §9.3.2: force expressions
// remain live, replacement removes the older binding, and release restores
// the correct persistent object.  The task targets a hierarchical variable.
`timescale 1ns/1ps
module child;
  reg [7:0] value;
endmodule

module tb;
  child u();
  reg [7:0] a, b;
  real rq, rd;

  task set_force_a;
    begin
      force u.value = a;
    end
  endtask

  task set_force_b;
    begin
      force u.value = b;
    end
  endtask

  task clear_force;
    begin
      release u.value;
    end
  endtask

  initial begin
    a = 8'h01;
    b = 8'h02;
    u.value = 8'h00;
    set_force_a();
    #1 $display("hier_initial=%h", u.value);
    a = 8'h03;
    #1 $display("hier_live=%h", u.value);
    set_force_b();
    b = 8'h06;
    a = 8'h07;
    #1 $display("hier_replaced=%h", u.value);
    clear_force();
    $display("hier_released=%h", u.value);
    u.value = 8'h04;
    $display("hier_write=%h", u.value);

    rq = 0.0;
    rd = 1.0;
    force rq = rd;
    rq <= 2.0;
    #1 $display("real_nba=%0.1f", rq);
    rd = 3.0;
    #1 $display("real_live=%0.1f", rq);
    release rq;
    $display("real_release=%0.1f", rq);
    $finish(0);
  end
endmodule
