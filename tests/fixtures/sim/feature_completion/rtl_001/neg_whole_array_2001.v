// V2001 3.10
// Expected: required diagnostic
module tb;
reg[7:0] a[0:1],b[0:1];
initial begin
a=b;
$finish;
end
endmodule
