// SV2009 11.4.11, 7.2.2
// Expected: 1  | 
module tb;
typedef struct {string s; int i;} T; T a,b,c; logic x;
initial begin
a='{"a",1}; b='{"b",1}; x=1'bx; c=x?a:b; $display("%0d %s",c.i,c.s);
$finish(0);
end
endmodule
