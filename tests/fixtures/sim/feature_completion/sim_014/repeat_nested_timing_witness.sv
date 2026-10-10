// SV2009 9.4.5, 12.7.2
// Expected: 7 3 | 7 7 | 
`timescale 1ns/1ns
module tb;
bit clk=0; int x; always #1 clk=~clk;
initial begin
repeat(2) begin x=repeat(2) @(posedge clk) 7; $display("%0d %0t",x,$time); end
$finish;
end
endmodule
