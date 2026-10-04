// SV2009 7.2.2, 7.6, 11.4.5
// Expected: 1 | 
module tb;
typedef struct {string s; real r;} T; T a,b;
initial begin
a='{"ok",1.5}; b=a; $display("%b",a==b);
$finish(0);
end
endmodule
