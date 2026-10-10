// SV2009 9.4.5, 10.4.2
// Expected: ok | 
module tb;
bit clk=0; string s; always #1 clk=~clk;
initial begin
s<=repeat(2) @(posedge clk) "ok"; #4; $display("%s",s);
$finish;
end
endmodule
