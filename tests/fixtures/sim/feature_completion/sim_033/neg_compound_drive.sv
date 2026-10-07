// SV2009 14.16
// Expected: required diagnostic
module tb;
bit clk; int x; clocking cb @(posedge clk); output x; endclocking
initial begin
cb.x+=1;
$finish;
end
endmodule
