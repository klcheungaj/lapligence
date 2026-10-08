// SV2009 14.5
// Expected: 7 | 
module tb;
bit clk=0; int a=3,b=4; clocking cb @(posedge clk); input sum=a+b; endclocking
initial begin
#1 clk=1; #1; $display("%0d",cb.sum);
$finish;
end
endmodule
