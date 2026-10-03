// SV2009 7.3.2
// Expected: required diagnostic
module tb;
union {string s; int n;} x;
initial begin

$finish;
end
endmodule
