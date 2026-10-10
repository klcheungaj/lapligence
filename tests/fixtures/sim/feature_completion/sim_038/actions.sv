// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_038/actions.sv
// SIM-038 A02: action blocks with timing controls. Every attempt of `pa`
// spawns its own action process, so overlapping pass and fail actions
// suspend and resume independently while new attempts keep starting. A
// procedural expect runs its action block in the calling process (16.18),
// assigns the caller's automatic output and resumes the caller exactly once
// per evaluation. Ticks 1..8 are the posedges at 5, 15, ..., 75; stimulus
// changes at 0, 10, ..., 70 and $finish ends the run at 80. The expected
// lines are derived in readme.md.
module tb;
  logic clk = 1'b0;
  logic s = 1'b0, b = 1'b0;
  // Bit 7 is tick 1, bit 0 is tick 8.
  logic [7:0] v_s = 8'b1100_0000, v_b = 8'b0110_0000;
  int resumes = 0;

  pa: assert property (@(posedge clk) s |=> (always [0:1] b))
    begin
      $display("%0t pa pass begin", $time);
      #12 $display("%0t pa pass end", $time);
    end
  else
    begin
      $display("%0t pa fail begin", $time);
      #7 $display("%0t pa fail end", $time);
    end

  task automatic wait_for(output bit ok);
    expect (@(posedge clk) s ##1 b)
      begin
        #3 ok = 1'b1;
      end
    else ok = 1'b0;
    resumes++;
  endtask

  initial begin
    bit ok;
    #2 wait_for(ok);
    $display("%0t e1 ok=%0d resumes=%0d", $time, ok, resumes);
    wait_for(ok);
    $display("%0t e2 ok=%0d resumes=%0d", $time, ok, resumes);
  end

  initial begin
    #30;
    expect (@(posedge clk) nexttime (always [0:1] !b))
      $display("%0t e3 pass", $time);
    else $display("%0t e3 fail", $time);
    $display("%0t e3 resumed", $time);
  end

  initial begin
    for (int i = 0; i < 8; i++) begin
      s = v_s[7 - i];
      b = v_b[7 - i];
      #5 clk = 1'b1;
      #5 clk = 1'b0;
    end
    $finish;
  end
endmodule
