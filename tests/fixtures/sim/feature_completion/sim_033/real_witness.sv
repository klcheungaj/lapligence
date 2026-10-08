// SV2009 14.3-14.5
// Expected: 1.25 | 
module tb;
bit clk=0; real r=1.25; clocking cb @(posedge clk); input r; endclocking
initial begin
#1 clk=1; #1; $display("%.2f",cb.r);
$finish;
end
endmodule
