// V2001 4.1.13; SV2009 11.4.11 Table11-20
// Expected: x |
module tb;
reg x; reg a,b;
initial begin
x=1'bx; a=1'bz; b=1'bz; $display("%b",x?a:b);
$finish;
end
endmodule
