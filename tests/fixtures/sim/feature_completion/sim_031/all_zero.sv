// All-zero weights select nothing: no body runs, execution continues after
// the statement, and each execution issues one warning on stderr.
module tb;
  int taken;
  logic [3:0] unknown = 4'bx0z0;
  logic [3:0] fully_unknown = 4'bxxxx;

  task automatic run(input int w0, input int w1);
    randcase
      w0 : taken++;
      w1 : taken++;
    endcase
  endtask

  initial begin
    repeat (3) run(0, 0);
    $display("after tasks taken=%0d", taken);
    repeat (2) begin
      randcase
        0 : taken++;
        0 : taken++;
        0 : taken++;
      endcase
    end
    $display("after constants taken=%0d", taken);
    repeat (2) begin
      randcase
        fully_unknown : taken++;
        4'b0000 : taken++;
      endcase
    end
    $display("after unknown taken=%0d", taken);
    // Z and X bits read as zero, so this weight is 0 as well.
    randcase
      unknown & 4'b0101 : taken++;
    endcase
    $display("done taken=%0d", taken);
    $finish;
  end
endmodule
