// rand join, IEEE 1800-2009 18.17.5: the sequences keep their relative order,
// the interleaving is random and the optional real expression weighs sequence
// lengths (decisions S16-D5 and S16-D6). Only sets of sequences, relations and
// a chi-square bound are printed, never a single generator-dependent trace.
module tb;
  timeunit 1ns;
  timeprecision 1ns;
  string trace;
  bit seen[string];
  int order_errors, first_counts[3];
  int results;
  real chi;
  int when[string];

  function automatic bit ordered(input string s, input string sub);
    // `sub` occurs in `s` as a subsequence.
    int j = 0;
    for (int i = 0; i < s.len() && j < sub.len(); i++)
      if (s[i] == sub[j]) j++;
    return j == sub.len();
  endfunction

  task automatic three_way(input real ratio);
    randsequence (top)
      top : rand join (ratio) s1 s2 s3 ;
      s1 : a1 a2 a3 ;
      s2 : b1 ;
      s3 : c1 c2 ;
      a1 : { trace = {trace, "a"}; } ;
      a2 : { trace = {trace, "A"}; } ;
      a3 : { trace = {trace, "@"}; } ;
      b1 : { trace = {trace, "b"}; } ;
      c1 : { trace = {trace, "c"}; } ;
      c2 : { trace = {trace, "C"}; } ;
    endsequence
  endtask

  task automatic two_way(input real ratio);
    randsequence (top)
      top : rand join (ratio) s1 s2 ;
      s1 : a1 a2 a3 ;
      s2 : b1 ;
      a1 : { trace = {trace, "a"}; } ;
      a2 : { trace = {trace, "A"}; } ;
      a3 : { trace = {trace, "@"}; } ;
      b1 : { trace = {trace, "b"}; } ;
    endsequence
  endtask

  initial begin
    // Three sequences of lengths 3, 1 and 2 at the default ratio: every
    // trace keeps each sequence's order, all 6!/(3!1!2!) = 60 interleavings
    // occur, and the first step is uniform (chi-square below 13.82, p=0.001
    // for 2 degrees of freedom).
    process::self().srandom(7);
    for (int i = 0; i < 6000; i++) begin
      trace = "";
      three_way(0.5);
      if (trace.len() != 6 || !ordered(trace, "aA@") || !ordered(trace, "b") ||
          !ordered(trace, "cC"))
        order_errors++;
      if (trace[0] == "a") first_counts[0]++;
      else if (trace[0] == "b") first_counts[1]++;
      else first_counts[2]++;
      seen[trace] = 1;
    end
    chi = 0.0;
    for (int i = 0; i < 3; i++) chi += (first_counts[i] - 2000.0) ** 2 / 2000.0;
    $display("0.5: order errors %0d, interleavings %0d, first step uniform %0d",
             order_errors, seen.num(), chi < 13.82);
    seen.delete();

    // Unequal lengths at the extremes produce exactly the prioritized sets.
    for (int seed = 1; seed <= 200; seed++) begin
      process::self().srandom(seed);
      trace = "";
      two_way(1.0);
      seen[trace] = 1;
    end
    $write("1.0:");
    foreach (seen[key]) $write(" %s", key);
    $display("");
    seen.delete();
    for (int seed = 1; seed <= 200; seed++) begin
      process::self().srandom(seed);
      trace = "";
      two_way(0.0);
      seen[trace] = 1;
    end
    $write("0.0:");
    foreach (seen[key]) $write(" %s", key);
    $display("");
    seen.delete();

    // An integral expression converts to real: (1) is 1.0.
    for (int seed = 1; seed <= 200; seed++) begin
      process::self().srandom(seed);
      trace = "";
      randsequence (top)
        top : rand join (1) s1 s2 ;
        s1 : a1 a2 a3 ;
        s2 : b1 ;
        a1 : { trace = {trace, "a"}; } ;
        a2 : { trace = {trace, "A"}; } ;
        a3 : { trace = {trace, "@"}; } ;
        b1 : { trace = {trace, "b"}; } ;
      endsequence
      seen[trace] = 1;
    end
    $write("integral 1:");
    foreach (seen[key]) $write(" %s", key);
    $display("");
    seen.delete();

    // Joined productions take arguments and return values; their results
    // are read in the weight code block of the joining rule. Each element of
    // a rule is one step: if, case, repeat and a nested join.
    for (int seed = 1; seed <= 100; seed++) begin
      process::self().srandom(seed);
      trace = "";
      randsequence (top)
        top : rand join p(2) q(5) r := 1 { results = p * 100 + q * 10 + r; } ;
        int p (int k) : x if (k > 1) y else z { return k; } ;
        int q (int k) : case (k) 5 : y ; default : z ; endcase repeat (2) x { return k; } ;
        int r : inner { return 7; } ;
        inner : rand join x y ;
        x : { trace = {trace, "x"}; } ;
        y : { trace = {trace, "y"}; } ;
        z : { trace = {trace, "z"}; } ;
      endsequence
      // p gives x y, q gives y x x, r gives one step x y or y x.
      if (results != 257 || trace.len() != 7 || ordered(trace, "z")) order_errors++;
      seen[trace] = 1;
    end
    $display("values: errors %0d, several interleavings %0d", order_errors, seen.num() > 1);
    seen.delete();

    // Interleaved code blocks may wait; the other sequences resume after.
    trace = "";
    process::self().srandom(5);
    randsequence (top)
      top : rand join slow fast ;
      slow : s1 s2 ;
      fast : f1 f2 ;
      s1 : { #3; when["s1"] = $time; } ;
      s2 : { #3; when["s2"] = $time; } ;
      f1 : { #1; when["f1"] = $time; } ;
      f2 : { #1; when["f2"] = $time; } ;
    endsequence
    $display("timed: total %0t, s1 before s2 %0d, f1 before f2 %0d", $time,
             when["s1"] < when["s2"], when["f1"] < when["f2"]);
    $finish;
  end
endmodule
