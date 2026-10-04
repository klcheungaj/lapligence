// IEEE 1800-2009 13.5.2, 7.10: legal (the FND-002 real_queue_ref witness
// expects 2.50). A ref to a real queue element needs a retained element cell
// that survives queue reallocation (SIM-008), so it is rejected explicitly.
module tb;
real q[$]; task automatic t(ref real x); x=2.5; endtask
initial begin
q.push_back(1.25); t(q[0]); $display("%.2f",q[0]);
$finish(0);
end
endmodule
