// SV2009 Annex A.6.2
// Expected: required diagnostic
module tb;
bit[7:0] a;
initial begin
{>>{a}}+=1;
$finish;
end
endmodule
