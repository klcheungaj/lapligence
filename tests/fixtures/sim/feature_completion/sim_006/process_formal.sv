// SV2009 9.7,13.5.1
// Expected: 1 | 
module tb;
task automatic t(input process p); $display("%0d",p.status()==process::RUNNING); endtask
initial begin
t(process::self());
$finish(0);
end
endmodule
