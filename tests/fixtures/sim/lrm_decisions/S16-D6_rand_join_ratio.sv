// Decision S16-D6: at every step the rand join expression r restricts the
// choice to the sequences with the most remaining elements with probability
// 2r - 1 when r > 0.5, and to those with the fewest remaining elements with
// probability 1 - 2r when r < 0.5; a sequence is then chosen uniformly among
// the candidates. At 0.0 and 1.0 the restriction always applies, so exactly
// the sequences the clause lists as having priority are produced; at 0.5
// every interleaving can occur. "Priority" is not otherwise quantified
// (llg choice).
//
// IEEE 1800-2009 18.17.5 (SystemVerilog-1800-2009.txt L31655-31669):
//   "The value of this expression represents the degree to which the length
//   of the sequences to be interleaved affects the probability of selecting
//   a sequence. A sequence's length is the number of productions not yet
//   interleaved at a given time. If the expression is 0.0, the shortest
//   sequences are given higher priority. If the expression is 1.0, the
//   longest sequences are given priority."
//   "If unspecified, the generator used the default value of 0.5, which does
//   not prioritize any sequence length."
module tb;
  string trace;
  int seen[string];
  task automatic generate_all(real ratio);
    seen.delete();
    for (int s = 1; s <= 200; s++) begin
      process::self().srandom(s);
      trace = "";
      randsequence (top)
        top : rand join (ratio) s1 s2 ;
        s1 : a b ;
        s2 : c d ;
        a : { trace = {trace, "A"}; } ;
        b : { trace = {trace, "B"}; } ;
        c : { trace = {trace, "C"}; } ;
        d : { trace = {trace, "D"}; } ;
      endsequence
      seen[trace] = 1;
    end
    foreach (seen[key]) $write(" %s", key);
    $display("");
  endtask
  initial begin
    $write("0.0:");
    generate_all(0.0);
    $write("1.0:");
    generate_all(1.0);
    $write("0.5:");
    generate_all(0.5);
    $finish;
  end
endmodule
