// Rule frequencies follow the weights of IEEE 1800-2009 18.17.1: "The
// probability that a particular production list is generated is proportional
// to its specified weight", with weight 1 where none is given. Each set runs
// under fixed seeds and passes when Pearson's chi-square stays below the
// p=0.001 critical value of its degrees of freedom (10.83 for 1, 13.82 for
// 2); zero-weight rules must never be generated. The seeds are fixed, so the
// result is reproducible.
module tb;
  int counts[3];
  int failures;
  int w;
  logic [39:0] wide_hi = 40'd3 << 36;
  logic [39:0] wide_lo = 40'd1 << 36;

  function automatic real chi2(input real p0, input real p1, input real p2);
    real n = counts[0] + counts[1] + counts[2];
    real p[3];
    real sum = 0.0;
    p[0] = p0;
    p[1] = p1;
    p[2] = p2;
    for (int i = 0; i < 3; i++)
      if (p[i] > 0.0) sum += (counts[i] - n * p[i]) ** 2 / (n * p[i]);
    return sum;
  endfunction

  task automatic check(input string set, input int seed, input real p0, input real p1,
                       input real p2, input real bound);
    real stat = chi2(p0, p1, p2);
    bit zero_hit = (p0 == 0.0 && counts[0] != 0) || (p1 == 0.0 && counts[1] != 0) ||
                   (p2 == 0.0 && counts[2] != 0);
    if (stat < bound && !zero_hit) begin
      $display("%s seed %0d: ok", set, seed);
    end else begin
      $display("%s seed %0d: FAIL chi2=%f counts %0d %0d %0d", set, seed, stat, counts[0],
               counts[1], counts[2]);
      failures = failures + 1;
    end
  endtask

  task automatic clear();
    counts[0] = 0;
    counts[1] = 0;
    counts[2] = 0;
  endtask

  initial begin
    for (int seed = 1; seed <= 3; seed++) begin
      // 18.17.1's own example: add 60 %, dec 40 %.
      process::self().srandom(seed);
      clear();
      repeat (10000) begin
        randsequence (first)
          first : add := 3 | dec := (1 + 1) ;
          add : { counts[0]++; } ;
          dec : { counts[1]++; } ;
        endsequence
      end
      check("lrm 3:2", seed, 0.6, 0.4, 0.0, 10.83);

      // No weights: every rule weighs 1.
      process::self().srandom(seed);
      clear();
      repeat (9000) begin
        randsequence (pick)
          pick : a | b | c ;
          a : { counts[0]++; } ;
          b : { counts[1]++; } ;
          c : { counts[2]++; } ;
        endsequence
      end
      check("default 1:1:1", seed, 1.0 / 3, 1.0 / 3, 1.0 / 3, 13.82);

      // Some weights given: the rest weigh 1.
      process::self().srandom(seed);
      clear();
      repeat (8000) begin
        randsequence (pick)
          pick : a := 5 | b | c := 2 ;
          a : { counts[0]++; } ;
          b : { counts[1]++; } ;
          c : { counts[2]++; } ;
        endsequence
      end
      check("mixed 5:1:2", seed, 5.0 / 8, 1.0 / 8, 2.0 / 8, 13.82);

      // A zero weight first, nonzero 1:3 after it.
      process::self().srandom(seed);
      clear();
      repeat (8000) begin
        randsequence (pick)
          pick : a := 0 | b := 1 | c := 3 ;
          a : { counts[0]++; } ;
          b : { counts[1]++; } ;
          c : { counts[2]++; } ;
        endsequence
      end
      check("zero 0:1:3", seed, 0.0, 0.25, 0.75, 10.83);

      // Weights read a variable each time the production is selected.
      process::self().srandom(seed);
      clear();
      for (int i = 0; i < 12000; i++) begin
        w = i % 3 == 0 ? 0 : 1;
        randsequence (pick)
          pick : a := w | b := (3 - 2 * w) ;
          a : { counts[0]++; } ;
          b : { counts[1]++; } ;
        endsequence
      end
      // w = 0 always gives b (4000); w = 1 gives a and b 1:1 (8000).
      counts[1] -= 4000;
      check("dynamic 1:1", seed, 0.5, 0.5, 0.0, 10.83);

      // 40-bit weights 3:1.
      process::self().srandom(seed);
      clear();
      repeat (8000) begin
        randsequence (pick)
          pick : a := wide_hi | b := wide_lo ;
          a : { counts[0]++; } ;
          b : { counts[1]++; } ;
        endsequence
      end
      check("wide40 3:1", seed, 0.75, 0.25, 0.0, 10.83);

      // A recursive production chooses its weighted rule at every level.
      process::self().srandom(seed);
      clear();
      repeat (2) begin
        randsequence (walk)
          walk : step(4000) ;
          void step(int k) : a next(k) := 1 | b next(k) := 3 ;
          next(int k) : if (k > 1) step(k - 1) ;
          a : { counts[0]++; } ;
          b : { counts[1]++; } ;
        endsequence
      end
      check("recursive 1:3", seed, 0.25, 0.75, 0.0, 10.83);
    end
    $display("failures=%0d", failures);
    $finish;
  end
endmodule
