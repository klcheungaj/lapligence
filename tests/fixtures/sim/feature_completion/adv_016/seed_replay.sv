// A fixed seed reproduces the same weighted production trace (IEEE 1800-2009
// 18.14 random stability; 18.17 draws its choices from the calling process).
// The trace itself depends on the random generator, so the expected output
// holds only relations: the same seed replays, another seed differs, and
// another process drawing at the same times does not change the trace. The
// checksum line is implementation dependent; the suite requires it to be
// identical across optimizer modes and value backends.
module tb;
  string trace;
  string saved_a, saved_b, saved_c, saved_d;
  event step;

  // Weighted rules, a repeat count, if, case, rand join and a recursive
  // production all draw from the generating process.
  task automatic produce(input int seed, input bit timed);
    process::self().srandom(seed);
    trace = "";
    randsequence (top)
      top : repeat ($urandom_range(8, 16)) unit tail(6) ;
      unit : alt := 3 | joined := 1 | choose := 2 ;
      alt : a := 1 | b := 2 | c := 5 ;
      joined : rand join (0.5) left right ;
      left : a b ;
      right : c pause a ;
      choose : case ($urandom_range(2)) 0 : a ; 1 : b ; default : c ; endcase ;
      tail(int k) : if (k > 0) more(k) ;
      more(int k) : a tail(k - 1) := 1 | b pause tail(k - 1) := 1 ;
      pause : { if (timed) begin
                  #1;
                  ->step;
                end } ;
      a : { trace = {trace, "a"}; } ;
      b : { trace = {trace, "b"}; } ;
      c : { trace = {trace, "c"}; } ;
    endsequence
  endtask

  function automatic int checksum(input string s);
    int sum = 0;
    for (int i = 0; i < s.len(); i++) sum = sum * 31 + int'(s[i]);
    return sum;
  endfunction

  initial begin
    produce(42, 1'b0);
    saved_a = trace;
    produce(42, 1'b0);
    saved_b = trace;
    produce(43, 1'b0);
    saved_c = trace;
    // The same generation with delays, while a second process draws from its
    // own generator at every step.
    fork
      produce(42, 1'b1);
      forever begin
        @step;
        void'($urandom);
        void'($urandom_range(9));
      end
    join_any
    disable fork;
    saved_d = trace;
    $display("replay equal: %s", saved_a == saved_b ? "yes" : "no");
    $display("other seed differs: %s", saved_a == saved_c ? "no" : "yes");
    $display("timed replay with another process equal: %s", saved_a == saved_d ? "yes" : "no");
    $display("checksum %0d %0d %0d", checksum(saved_a), checksum(saved_c), saved_a.len());
    $finish;
  end
endmodule
