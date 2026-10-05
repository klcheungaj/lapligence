`define CALL(t) t()
module tb;
  task ping;
    $display("ping");
  endtask
  initial `CALL(ping);
endmodule
