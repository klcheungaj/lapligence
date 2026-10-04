// SV2009 10.3, 6.16
// Expected: new | 
module tb;
string a="old", b; assign b=a;
initial begin
#1; a="new"; #1; $display("%s",b);
$finish(0);
end
endmodule
