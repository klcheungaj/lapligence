// Decision S45-D10: a `default sequence` bin counts at a sample when no
// other transition bin of the coverpoint completes a sequence there (counted
// or not) and no attempt that was pending before the sample still is. It
// does not count at the first sample (no transition ends there), after a
// coverpoint iff or stop() restarts the history (S45-D9), or at a sample with
// X or Z bits (S45-D7). Here the default is illegal, so each count is a
// run-time error, and the run exits with a failure status: the errors come
// at samples 3 and 5 of the clause's example (values 7 and 8), and at samples
// 2, 3 and 6 of `q` (values 9, 1 and 9; like the clause's 7, the 1 starts a
// sequence but ends none).
//
// IEEE 1800-2009 19.5.1 (SystemVerilog-1800-2009.txt L32672-32684): "The bin
// allother is incremented when none of the coverpoint's other nondefault
// sequence transition bins increments and none of the coverpoint's
// previously pending transition bins remains pending. ... The bin allother
// increments twice. The bin allother increments on the sample of 7 ... The
// bin allother increments on the sample of 8". The example's first sample
// starts a pending sequence; the text is silent on a first sample that does
// not.
module tb;
  bit [4:1] v_a;
  logic [3:0] w;
  covergroup cg;
    coverpoint v_a {
      bins sa = (4 => 5 => 6), ([7:9],10 => 11,12);
      bins sc = (12 => 3 [-> 1]);
      illegal_bins allother = default sequence;
    }
  endgroup
  covergroup cq;
    q: coverpoint w {
      bins t = (1 => 2);
      illegal_bins other = default sequence;
    }
  endgroup
  cg c = new;
  cq d = new;
  bit [4:1] values[9] = '{4, 5, 7, 11, 8, 12, 2, 2, 3};
  logic [3:0] q_values[6] = '{9, 9, 1, 2, 4'bx, 9};
  initial begin
    foreach (values[i]) begin
      v_a = values[i];
      $display("v_a sample %0d", i + 1);
      c.sample();
    end
    foreach (q_values[i]) begin
      w = q_values[i];
      $display("w sample %0d", i + 1);
      d.sample();
    end
    $finish;
  end
endmodule
