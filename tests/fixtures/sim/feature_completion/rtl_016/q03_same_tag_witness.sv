// SV2009 4.9.4,10.4.2,11.9
// Adopted FND-002 witness q03_same_tag. Expected: 7
module tb;
typedef union tagged packed {logic[7:0] A; logic[7:0] B;} T; T x;
initial begin
x=tagged A 1; x.A<=7; x=tagged A 2; #1; $display("%0d",x.A);
$finish;
end
endmodule
