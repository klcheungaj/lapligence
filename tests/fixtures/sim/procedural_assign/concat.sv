`timescale 1ns/1ps
module tb;
  reg [3:0] high;
  reg low;
  reg [4:0] source;
  initial begin
    high = 0;
    low = 0;
    source = 5'b10110;
    assign {high, low} = source;
    high = 0;
    low = 0;
    #0 $display("CHECK: concat_blocking=%b%b", high, low);
    high <= 0;
    low <= 0;
    #1 $display("CHECK: concat_nba=%b%b", high, low);
    source = 5'b01001;
    #1 $display("CHECK: concat_live=%b%b", high, low);
    force high = 4'b1111;
    source = 5'b11011;
    #1 $display("CHECK: concat_forced=%b%b", high, low);
    release high;
    #0 $display("CHECK: concat_released=%b%b", high, low);
    deassign {high, low};
    high = 0;
    low = 0;
    $display("CHECK: concat_deassign=%b%b", high, low);
    $finish(0);
  end
endmodule
