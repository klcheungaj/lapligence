// Decision S31-D3: a zero-weight branch is never taken, whatever its
// position, and when every weight is zero no branch is taken and execution
// continues with the next statement. llg issues a warning on stderr for the
// all-zero case; the warning is optional in the standard and not part of the
// compared output.
//
// IEEE 1800-2009 18.16 (SystemVerilog-1800-2009.txt L31332-31333):
//   "If a branch specifies a zero weight, then that branch is not taken. If
//   all randcase_items specify zero weights, then no branch is taken and a
//   warning can be issued."
//
// The random number is drawn from 0 to sum - 1, so a leading, middle or
// trailing zero-weight branch is never reached at either end of the range.
module tb;
  int w0 = 0;
  int w1 = 5;
  int w2 = 0;
  int w3 = 7;
  int w4 = 0;
  int count[5];
  int z = 0;
  int taken;
  initial begin
    repeat (600) begin
      randcase
        w0 : count[0]++;
        w1 : count[1]++;
        w2 : count[2]++;
        w3 : count[3]++;
        w4 : count[4]++;
      endcase
    end
    $display("zero-weight branches taken: %0d", count[0] + count[2] + count[4]);
    $display("picks: %0d", count[1] + count[3]);
    $display("both nonzero branches taken: %s", (count[1] > 0 && count[3] > 0) ? "yes" : "no");
    repeat (10) begin
      randcase
        z : taken++;
        z : taken++;
      endcase
    end
    $display("all-zero branches taken: %0d", taken);
    $display("continues: yes");
    $finish;
  end
endmodule
