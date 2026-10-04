// SV2009 9.2.2.4
// Expected: required diagnostic
module tb;
logic c,d,x; always_ff begin @(posedge c); @(posedge d); x=1; end
initial begin

$finish;
end
endmodule
