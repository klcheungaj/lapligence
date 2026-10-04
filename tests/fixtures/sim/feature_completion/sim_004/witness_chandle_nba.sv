// SV2009 6.14, 10.4.2
// Expected: 1 | 
module tb;
chandle a,b;
initial begin
a=null; b<=a; #1; $display("%0d",b==null);
$finish(0);
end
endmodule
