// SV2009 23.11, 24
// Expected: 7 | 
program p(input int x); initial $display("%0d",x); endprogram
module tb; int x=7; endmodule
bind tb p u(x);
