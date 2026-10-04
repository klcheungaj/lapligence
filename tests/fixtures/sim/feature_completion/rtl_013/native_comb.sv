// SV2009 9.2.2.2
// Expected: 3 |
module tb;
typedef struct {string s; int i;} T; T a; int n; always_comb n=a.s.len()+a.i;
initial begin
a.s="ab"; a.i=1; #1; $display("%0d",n);
$finish;
end
endmodule
