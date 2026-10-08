// SV2009 14.16
// Expected: required diagnostic
module tb;
bit clk; int x,y; clocking cb @(posedge clk); output x,y; endclocking
initial begin
{cb.x,cb.y}<=0;
$finish;
end
endmodule
