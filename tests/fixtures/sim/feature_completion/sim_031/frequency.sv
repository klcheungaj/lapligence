// Frequency check. Each set draws N times per seed from fixed seeds and
// compares the observed counts with the weights through Pearson's chi-square
// statistic. The bound is the p=0.001 critical value for the degrees of
// freedom (nonzero branches minus one), so a correct implementation fails one
// seed in a thousand by chance; the seeds are fixed, so the outcome is
// reproducible. The statistic itself is not printed (it depends on the
// generator); a failing set prints it.
module tb;
  localparam int SEEDS = 5;
  localparam real CRIT_DF1 = 10.83;
  localparam real CRIT_DF2 = 13.82;

  int failures;
  real chi;
  int counts[4];

  function automatic real chi_square(input real expected[4]);
    real total = 0.0;
    for (int i = 0; i < 4; i++) begin
      if (expected[i] > 0.0) begin
        real d = counts[i] - expected[i];
        total += d * d / expected[i];
      end
    end
    return total;
  endfunction

  task automatic report(input string name, input int seed, input real crit, input real stat,
                        input int zero_hits);
    if (stat < crit && zero_hits == 0) $display("%s seed %0d: ok", name, seed);
    else begin
      $display("%s seed %0d: FAIL chi2=%f bound=%f zero_hits=%0d", name, seed, stat, crit,
               zero_hits);
      failures++;
    end
  endtask

  // Small constant weights 3:1:0:4 (df 2).
  task automatic set_small(input int seed);
    localparam int N = 40000;
    real expected[4];
    int w0 = 3, w1 = 1, w2 = 0, w3 = 4;
    counts = '{0, 0, 0, 0};
    void'($urandom(seed));
    repeat (N) begin
      randcase
        w0 : counts[0]++;
        w1 : counts[1]++;
        w2 : counts[2]++;
        w3 : counts[3]++;
      endcase
    end
    expected = '{N * 3.0 / 8.0, N * 1.0 / 8.0, 0.0, N * 4.0 / 8.0};
    report("small 3:1:0:4", seed, CRIT_DF2, chi_square(expected), counts[2]);
  endtask

  // Skewed 1:99 (df 1).
  task automatic set_skewed(input int seed);
    localparam int N = 40000;
    real expected[4];
    counts = '{0, 0, 0, 0};
    void'($urandom(seed));
    repeat (N) begin
      randcase
        1 : counts[0]++;
        99 : counts[1]++;
      endcase
    end
    expected = '{N * 0.01, N * 0.99, 0.0, 0.0};
    report("skewed 1:99", seed, CRIT_DF1, chi_square(expected), 0);
  endtask

  // 40-bit weights: the sum needs more than 32 bits, so the draw combines two
  // 32-bit values (df 1).
  task automatic set_wide40(input int seed);
    localparam int N = 20000;
    real expected[4];
    logic [39:0] wa = 40'd3 << 32;
    logic [39:0] wb = 40'd1 << 32;
    counts = '{0, 0, 0, 0};
    void'($urandom(seed));
    repeat (N) begin
      randcase
        wa : counts[0]++;
        wb : counts[1]++;
      endcase
    end
    expected = '{N * 0.75, N * 0.25, 0.0, 0.0};
    report("wide40 3:1", seed, CRIT_DF1, chi_square(expected), 0);
  endtask

  // 100-bit weights exceed 64 bits and use the arbitrary-width path (df 1).
  task automatic set_wide100(input int seed);
    localparam int N = 8000;
    real expected[4];
    logic [99:0] wa = 100'd3 << 90;
    logic [99:0] wb = 100'd1 << 90;
    counts = '{0, 0, 0, 0};
    void'($urandom(seed));
    repeat (N) begin
      randcase
        wa : counts[0]++;
        wb : counts[1]++;
      endcase
    end
    expected = '{N * 0.75, N * 0.25, 0.0, 0.0};
    report("wide100 3:1", seed, CRIT_DF1, chi_square(expected), 0);
  endtask

  // Unequal widths: a 3-bit weight beside a 70-bit one (df 1). The 70-bit
  // weight is 2*7 so the shares are 7:14.
  task automatic set_mixed_widths(input int seed);
    localparam int N = 8000;
    real expected[4];
    logic [2:0] wa = 3'd7;
    logic [69:0] wb = 70'd14;
    counts = '{0, 0, 0, 0};
    void'($urandom(seed));
    repeat (N) begin
      randcase
        wa : counts[0]++;
        wb : counts[1]++;
      endcase
    end
    expected = '{N * 7.0 / 21.0, N * 14.0 / 21.0, 0.0, 0.0};
    report("mixed 7:14", seed, CRIT_DF1, chi_square(expected), 0);
  endtask

  initial begin
    for (int seed = 1; seed <= SEEDS; seed++) begin
      set_small(seed);
      set_skewed(seed);
      set_wide40(seed);
      set_wide100(seed);
      set_mixed_widths(seed);
    end
    $display("failures=%0d", failures);
    $finish;
  end
endmodule
