// SV2009 9.4.2, 8.4
// Expected: changed |
module tb;
class C; endclass C a;
initial begin
fork begin @(a); $display("changed"); end begin #1; a=new; end join
$finish;
end
endmodule
