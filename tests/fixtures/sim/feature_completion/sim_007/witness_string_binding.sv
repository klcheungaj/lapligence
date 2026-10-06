// SV2009 12.6
// Expected: ok | 
module tb;
string a="ok";
initial begin
if(a matches .s) $display("%s",s);
$finish(0);
end
endmodule
