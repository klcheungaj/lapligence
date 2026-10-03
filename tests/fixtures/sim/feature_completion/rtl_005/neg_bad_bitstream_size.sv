// SV2009 6.24.3
// Expected: required diagnostic
module tb;
typedef int A[2]; A a;
initial begin
a=A'(32'h7);
$finish;
end
endmodule
