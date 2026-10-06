// Adopted FND-002 witness timed_class (ledger L-F09-07-03, L-F12-01-02).
// SV2009 8.6,13.3
// Expected: class |
module tb;
class C; task t(); #1; $display("class"); endtask endclass C c;
initial begin
c=new; c.t();
$finish;
end
endmodule
