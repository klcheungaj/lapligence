// SV2009 11.4.2
// Expected: required diagnostic
module tb;
int a=1,b=2;
initial begin
(a+b)++;
$finish(0);
end
endmodule
