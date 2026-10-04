// SV2009 10.4.2, 6.16, 6.21
// Expected: new | 
module tb;
string s;
initial begin
s="old"; s<="new"; #1; $display("%s",s);
$finish(0);
end
endmodule
