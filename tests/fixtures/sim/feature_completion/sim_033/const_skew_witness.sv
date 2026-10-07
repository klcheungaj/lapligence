// SV2009 14.4-14.5
// Expected: 1 | 
module tb;
bit clk=0,x=1; always #1 clk=~clk; clocking cb @(posedge clk); input #1step x; endclocking
initial begin
@(cb); $display("%b",cb.x);
$finish;
end
endmodule
