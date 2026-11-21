// A class task a process calls writes `s`, which a continuous assignment
// drives. See readme.md.
`timescale 1ns/1ps
module tb;
  logic x = 1'b0;
  logic s;
  assign s = x;
  class D;
    task run();
      #12 s = 1'b1;
    endtask
  endclass
  initial begin
    static D h = new;
    fork h.run(); join_none
    #13 $display("s=%b", s);
    $finish;
  end
endmodule
