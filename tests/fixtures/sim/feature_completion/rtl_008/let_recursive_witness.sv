// SV2009 11.13
// Expected: required diagnostic
module tb;
let f(x)=f(x);
initial begin
$display("%0d",f(1));
$finish;
end
endmodule
