`timescale 1ns/1ns
// Edge input skews in every clocking event form llg serves (SV 14.3): a
// negedge skew under a posedge event, a posedge skew under a negedge event,
// a negedge skew under an any-edge event, a block default edge skew, a
// four-state input before any skew edge, a real input, an unpacked input,
// and a read between the skew edge and the clocking event. One process
// prints at every clock edge after all blocks published.
module tb;
  bit clk = 0;
  always #5 clk = ~clk;
  int x = 0, y = 0;
  logic [3:0] w;
  real r = 1.5;
  int arr [2] = '{1, 2};
  clocking cb @(posedge clk);
    input negedge x, w, r, arr;
  endclocking
  clocking cn @(negedge clk);
    input posedge p = x;
  endclocking
  clocking ca @(clk);
    input negedge q = y;
  endclocking
  clocking cd @(posedge clk);
    default input negedge;
    input dy = y;
  endclocking
  initial begin
    #7  begin x = 1; y = 1; end
    #5  begin x = 2; y = 2; w = 4'h3; r = 2.5; arr[1] = 20; end
    #10 begin x = 3; y = 3; end
    #2  begin x = 4; y = 4; w = 4'h4; r = 3.5; arr[0] = 10; end
  end
  initial begin
    #1 $display("%0t before x %0d w %h r %0.1f arr %0d p %0d q %0d dy %0d", $time, cb.x, cb.w,
                cb.r, cb.arr[1], cn.p, ca.q, cd.dy);
    #11 $display("%0t between x %0d w %h r %0.1f arr %0d dy %0d", $time, cb.x, cb.w, cb.r,
                 cb.arr[1], cd.dy);
  end
  always @(ca)
    $display("%0t clk %0d x %0d w %h r %0.1f arr %0d %0d p %0d q %0d dy %0d", $time, clk, cb.x,
             cb.w, cb.r, cb.arr[0], cb.arr[1], cn.p, ca.q, cd.dy);
  initial #36 $finish;
endmodule
