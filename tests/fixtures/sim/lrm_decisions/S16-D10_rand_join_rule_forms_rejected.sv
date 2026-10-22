// Decision S16-D10: a production that rand join interleaves must have exactly
// one rule, made of production items and code blocks. A joined production
// with several rules, or whose rule has an if, case or repeat item, is a
// compile error located at the rand join item (here `s1`, which has two
// rules). Productions it generates further down are not restricted. The
// expected output is empty: the design does not compile.
//
// IEEE 1800-2009 18.17.5 (SystemVerilog-1800-2009.txt L31615-31616, L31671):
//   "The rand join production control is used to randomly interleave two or
//   more production sequences while maintaining the relative order of each
//   sequence." "At each step, the generator interleaves nonterminal symbols
//   to depth of 1."
//
// The text shows only productions of one rule of production items and does
// not say how alternatives or if, case and repeat items take part in the
// interleaving; llg rejects them, the industry-standard behaviour.
module tb;
  initial begin
    randsequence (top)
      top : rand join s1 s2 ;
      s1 : a b | b a ;
      s2 : a ;
      a : { $display("a"); } ;
      b : { $display("b"); } ;
    endsequence
    $finish;
  end
endmodule
