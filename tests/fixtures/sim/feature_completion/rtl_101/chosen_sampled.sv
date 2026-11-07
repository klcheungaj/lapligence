// Sampled values of a record's member array. Rebuild with N = 8 to read the
// same design through per-cell leaves: the output is the same.
`timescale 1ns / 1ns
`ifndef N
`define N 16
`endif
module tb;
  typedef struct {
    logic [7:0] a[`N];
    int s;
  } rec_t;
  rec_t r, q, u;
  logic clk = 0;
  int k = 3;
  always #5 clk = ~clk;

  // A write in the slot of the clock edge must not reach the sample, and a
  // change of every cell, written or not, keeps the slot's old values.
  initial begin
    r.a[3] = 8'h01;
    q.a[3] = 8'h40;
    #5 r.a[3] = 8'h02;
    #10 r.a[3] = 8'h03;
    r.a[4] = 8'h81;
    #3 r.a[3] = 8'h07;
    r.a[3] = 8'h09;
    #7 k = 4;
    #10 r = q;
    #10 r.a = '{default: 8'h55};
    r.a[6] = 8'h66;
    #10 r.a[4] = 8'h00;
    #10 $finish(0);
  end

  // A nonblocking write lands after the sample of its own slot.
  initial u.a[2] = 8'h10;
  always @(posedge clk) u.a[2] <= u.a[2] + 8'h01;

  always @(posedge clk)
    $display("%0t s=%h b=%b p=%h r=%0d c=%b f=%b st=%b q=%h %h u=%h d=%h %h", $time,
             $sampled(r.a[3]), $sampled(r.a[4][7]), $sampled(r.a[3][3:0]),
             $rose(r.a[3][0]), $changed(r.a[3]), $fell(r.a[4][7]),
             $stable(r.a[k]), $past(r.a[k]), $past(r.a[3], 2), $sampled(u.a[2]),
             $sampled(r.a[7]), $sampled(r.a[6]));

  property follows;
    @(posedge clk) r.a[3] != 8'h00 |=> r.a[k] != 8'hee;
  endproperty
  a_follows: assert property (follows)
    $display("%0t follows", $time);
  else
    $display("%0t misses", $time);

  a_cell: assert property (@(posedge clk) r.a[k] == 8'h81)
    $display("%0t cell", $time);
  else
    $display("%0t no cell", $time);
endmodule
