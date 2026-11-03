// SV2009 9.4.5, 10.4.2
// Expected: t2 [] | t4 [ok]
module tb;
bit clk=0; string s; always #1 clk=~clk;
initial begin
s<=repeat(2) @(posedge clk) "ok"; #2 $display("t2 [%s]",s); #2 $display("t4 [%s]",s);
$finish;
end
endmodule
