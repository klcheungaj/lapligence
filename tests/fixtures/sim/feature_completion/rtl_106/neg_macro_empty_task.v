`define NO_ARGS ()
module tb;
  task ping `NO_ARGS;
    $display("ping");
  endtask
endmodule
