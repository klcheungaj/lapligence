// SIM-009: an expanded task's event control on a `ref` formal bound to an
// array element waits on the element selected when the call starts (SV
// 13.5.2): a later index change and stores to other elements do not wake it.
module tb;
  logic bus [3];
  task automatic await_edge(ref logic s);
    @(posedge s);
    $display("edge %0d", $time);
  endtask
  initial begin
    automatic int i = 2;
    bus[0] = 0; bus[1] = 0; bus[2] = 0;
    fork await_edge(bus[i]); join_none
    #1 i = 0;
    #1 bus[0] = 1;
    #1 bus[2] = 1;
    #1 $finish;
  end
endmodule
