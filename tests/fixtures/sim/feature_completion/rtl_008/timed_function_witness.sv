// SV2009 13.4.4
// Expected: required diagnostic
module tb;
function int f(); #1; return 7; endfunction
initial begin
 $display("%0d",f());
$finish;
end
endmodule
