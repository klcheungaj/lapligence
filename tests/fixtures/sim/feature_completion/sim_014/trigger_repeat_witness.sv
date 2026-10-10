// SV2009 15.5.1, Annex A.6.5
// Expected: event | 
module tb;
bit clk=0; event e; always #1 clk=~clk;
initial begin
fork begin @e; $display("event"); end begin ->>repeat(2) @(posedge clk) e; end join
$finish;
end
endmodule
