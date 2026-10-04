// SV2009 10.4.2, 7.2.2, 6.21
// Expected: ok 7 | 
module tb;
typedef struct {string s; int i;} T; T a,b;
initial begin
a='{"ok",7}; b<=a; #1; $display("%s %0d",b.s,b.i);
$finish(0);
end
endmodule
