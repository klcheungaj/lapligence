// SV2009 7.12.2
// Expected: required diagnostic
module tb;
int a[2];
initial begin
a.reverse() with (item);
$finish;
end
endmodule
