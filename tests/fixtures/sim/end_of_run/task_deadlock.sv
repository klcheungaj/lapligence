// A task called from an initial procedure blocks the initial procedure.
module tb;
  event ev;
  task automatic wait_forever;
    @(ev);
  endtask
  initial begin
    wait_forever();
    $display("never printed");
  end
  final $display("final");
endmodule
