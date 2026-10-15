// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_039/named_action_blocks.sv
// Named and declaring action blocks on module-level and generate-level
// concurrent assertions, labeled and unlabeled. Every line carries its time;
// see the fixture readme for the hand-derived expectations.
module tb;
  logic clk = 1'b0, a = 1'b1, b = 1'b0;

  sequence s;
    a;
  endsequence

  always #5 clk = ~clk;

  initial begin
    #10 b = 1'b1;
    #10 a = 1'b0;
    #10 a = 1'b1;
    b = 1'b0;
    #10 $finish;
  end

  ma: assert property (@(posedge clk) a) begin : pass_blk
    static int n = 0;
    n++;
    begin : inner
      $display("%0d ma pass %m n=%0d", $time, n);
    end
  end else begin : fail_blk
    $display("%0d ma fail %m", $time);
  end

  md: assert property (@(posedge clk) b) else begin : d_blk
    $display("%0d md fail %m", $time);
    disable d_blk;
    $display("%0d md not disabled", $time);
  end

  mc: cover sequence (@(posedge clk) s) begin : c_blk
    #1 $display("%0d mc %m", $time);
  end

  mu: assume property (@(posedge clk) b) else begin : u_blk
    $display("%0d mu fail %m", $time);
  end

  cover property (@(posedge clk) a ##1 b) begin : ua_blk
    $display("%0d ua %m", $time);
  end

  ld: cover property (@(posedge clk) b) begin
    int w;
    w = 2;
    for (int k = 0; k < w; k++) $display("%0d ld k=%0d", $time, k);
  end

  if (1) begin : g
    gc: cover property (@(posedge clk) s) begin : act
      $display("%0d gc %m", $time);
    end
    ga: assume property (@(posedge clk) a) else begin : act
      $display("%0d ga fail %m", $time);
    end
    for (genvar i = 0; i < 2; i++) begin : n
      gs: cover sequence (@(posedge clk) b ##1 a) begin : act
        $display("%0d gs i=%0d %m", $time, i);
      end
    end
  end
endmodule
