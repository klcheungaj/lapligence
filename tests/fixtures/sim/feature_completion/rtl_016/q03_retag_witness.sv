// SV2009 4.9.4,10.4.2,7.3.2,11.9
// Adopted FND-002 witness q03_retag; RTL-016 resolves its commit check:
// the queued A write finds tag B at commit, reports a run-time error and
// leaves the B value unchanged. Results go to stderr with the diagnostic.
module tb;
localparam logic [31:0] STDERR = 32'h8000_0002;
typedef union tagged packed {logic[7:0] A; logic[7:0] B; void Empty;} T; T x;
initial begin
x=tagged A 1; x.A<=7; x=tagged B 2; #1; $fdisplay(STDERR, "%b",x);
$finish;
end
endmodule
