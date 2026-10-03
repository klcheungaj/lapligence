// SV2009 7.3.1
// Expected: required diagnostic
module tb;
union packed {bit[3:0] a; bit[7:0] b;} x;
initial begin

$finish;
end
endmodule
