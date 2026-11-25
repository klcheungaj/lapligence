// Each task calls the next one at two places, so `t0` reaches `t30` along 2^30
// call paths and a frame type names the one it calls twice. The second call of
// each task passes an argument that ends there, so only 61 tasks run.
`timescale 1ns/1ns
module tb;
  int y;

  task automatic t30(input int x); #1 y = y + x; endtask
  task automatic t29(input int x); if (x == 200) begin t30(x); #1 t30(x + 1); end else #1 y = y + x; endtask
  task automatic t28(input int x); if (x == 200) begin t29(x); #1 t29(x + 1); end else #1 y = y + x; endtask
  task automatic t27(input int x); if (x == 200) begin t28(x); #1 t28(x + 1); end else #1 y = y + x; endtask
  task automatic t26(input int x); if (x == 200) begin t27(x); #1 t27(x + 1); end else #1 y = y + x; endtask
  task automatic t25(input int x); if (x == 200) begin t26(x); #1 t26(x + 1); end else #1 y = y + x; endtask
  task automatic t24(input int x); if (x == 200) begin t25(x); #1 t25(x + 1); end else #1 y = y + x; endtask
  task automatic t23(input int x); if (x == 200) begin t24(x); #1 t24(x + 1); end else #1 y = y + x; endtask
  task automatic t22(input int x); if (x == 200) begin t23(x); #1 t23(x + 1); end else #1 y = y + x; endtask
  task automatic t21(input int x); if (x == 200) begin t22(x); #1 t22(x + 1); end else #1 y = y + x; endtask
  task automatic t20(input int x); if (x == 200) begin t21(x); #1 t21(x + 1); end else #1 y = y + x; endtask
  task automatic t19(input int x); if (x == 200) begin t20(x); #1 t20(x + 1); end else #1 y = y + x; endtask
  task automatic t18(input int x); if (x == 200) begin t19(x); #1 t19(x + 1); end else #1 y = y + x; endtask
  task automatic t17(input int x); if (x == 200) begin t18(x); #1 t18(x + 1); end else #1 y = y + x; endtask
  task automatic t16(input int x); if (x == 200) begin t17(x); #1 t17(x + 1); end else #1 y = y + x; endtask
  task automatic t15(input int x); if (x == 200) begin t16(x); #1 t16(x + 1); end else #1 y = y + x; endtask
  task automatic t14(input int x); if (x == 200) begin t15(x); #1 t15(x + 1); end else #1 y = y + x; endtask
  task automatic t13(input int x); if (x == 200) begin t14(x); #1 t14(x + 1); end else #1 y = y + x; endtask
  task automatic t12(input int x); if (x == 200) begin t13(x); #1 t13(x + 1); end else #1 y = y + x; endtask
  task automatic t11(input int x); if (x == 200) begin t12(x); #1 t12(x + 1); end else #1 y = y + x; endtask
  task automatic t10(input int x); if (x == 200) begin t11(x); #1 t11(x + 1); end else #1 y = y + x; endtask
  task automatic t9(input int x); if (x == 200) begin t10(x); #1 t10(x + 1); end else #1 y = y + x; endtask
  task automatic t8(input int x); if (x == 200) begin t9(x); #1 t9(x + 1); end else #1 y = y + x; endtask
  task automatic t7(input int x); if (x == 200) begin t8(x); #1 t8(x + 1); end else #1 y = y + x; endtask
  task automatic t6(input int x); if (x == 200) begin t7(x); #1 t7(x + 1); end else #1 y = y + x; endtask
  task automatic t5(input int x); if (x == 200) begin t6(x); #1 t6(x + 1); end else #1 y = y + x; endtask
  task automatic t4(input int x); if (x == 200) begin t5(x); #1 t5(x + 1); end else #1 y = y + x; endtask
  task automatic t3(input int x); if (x == 200) begin t4(x); #1 t4(x + 1); end else #1 y = y + x; endtask
  task automatic t2(input int x); if (x == 200) begin t3(x); #1 t3(x + 1); end else #1 y = y + x; endtask
  task automatic t1(input int x); if (x == 200) begin t2(x); #1 t2(x + 1); end else #1 y = y + x; endtask
  task automatic t0(input int x); if (x == 200) begin t1(x); #1 t1(x + 1); end else #1 y = y + x; endtask

  initial begin
    t0(200);
    $display("%0t %0d", $time, y);
    $finish(0);
  end
endmodule
