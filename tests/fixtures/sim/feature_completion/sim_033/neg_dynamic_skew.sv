// SV2009 14.4
// Expected: required diagnostic
module tb;
bit clk; int n=1,x; clocking cb @(posedge clk); input #(n) x; endclocking
initial begin

$finish;
end
endmodule
