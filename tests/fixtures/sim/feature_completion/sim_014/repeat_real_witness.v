// V2001 3.9, 9.6, 9.7.7; SV2009 6.12, 9.4.5, 12.7.2
// Expected: 7 | 
module tb;
reg clk=0; integer x; real n=2.0; always #1 clk=~clk;
initial begin
x=repeat(n) @(posedge clk) 7; $display("%0d",x);
$finish;
end
endmodule
