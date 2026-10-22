// Decision S16-D5: each production that rand join interleaves contributes its
// one rule's elements as one sequence. Each element (a production item or a
// code block) is one step and is generated whole, so a nonterminal is
// expanded to depth 1 only. The rule's weight code block is its last element
// (S16-D2) and one step of its own. A return aborts the rest of its own
// sequence (the production whose code block runs it) while the other
// sequences continue. The clause gives the steps only by example (llg
// choice). A joined production with several rules, or with an if, case or
// repeat item, is rejected (S16-D10).
//
// IEEE 1800-2009 18.17.5 (SystemVerilog-1800-2009.txt L31615-31616):
//   "The rand join production control is used to randomly interleave two or
//   more production sequences while maintaining the relative order of each
//   sequence."
// IEEE 1800-2009 18.17.5 (L31671):
//   "At each step, the generator interleaves nonterminal symbols to depth of
//   1."
// IEEE 1800-2009 18.17.6 (L31693-31695):
//   "The return statement aborts the generation of the current production.
//   When a return statement is executed from within a production code block,
//   the current production is aborted. Sequence generation continues with the
//   next production following the aborted production."
module tb;
  string trace;
  int order_errors, whole_errors, weight_errors, return_errors;
  int at_w;
  initial begin
    for (int s = 1; s <= 200; s++) begin
      process::self().srandom(s);
      trace = "";
      randsequence (top)
        top : rand join s1 s2 s3 ;
        s1 : a b := 1 { trace = {trace, "w"}; } ;
        s2 : pair d ;
        s3 : e f ;
        pair : c1 c2 ;
        a : { trace = {trace, "a"}; } ;
        b : { trace = {trace, "b"}; } ;
        c1 : { trace = {trace, "c"}; } ;
        c2 : { trace = {trace, "C"}; } ;
        d : { trace = {trace, "d"}; } ;
        e : { trace = {trace, "e"}; } ;
        f : { trace = {trace, "f"}; } ;
      endsequence
      // Sequence order holds: a before b before w, pair before d, e before f.
      for (int i = 0; i < trace.len(); i++)
        for (int j = i + 1; j < trace.len(); j++)
          if ((trace[i] == "b" && trace[j] == "a") ||
              (trace[i] == "w" && trace[j] == "b") ||
              (trace[i] == "d" && trace[j] == "c") ||
              (trace[i] == "f" && trace[j] == "e"))
            order_errors = order_errors + 1;
      // The pair element is generated whole: c is always followed by C.
      for (int i = 0; i < trace.len(); i++)
        if (trace[i] == "c" && (i + 1 >= trace.len() || trace[i + 1] != "C"))
          whole_errors = whole_errors + 1;
      // Every step runs once; the weight block is a step of its own, so it
      // sometimes comes later than right after b.
      if (trace.len() != 8) weight_errors = weight_errors + 1;
      for (int i = 0; i + 1 < trace.len(); i++)
        if (trace[i] == "b" && trace[i + 1] != "w") at_w = at_w + 1;
    end
    $display("order=%0d whole=%0d weight=%0d separate=%0d", order_errors, whole_errors,
             weight_errors, at_w > 0);
    // The return in s3's code block drops f and nothing else.
    for (int s = 1; s <= 200; s++) begin
      process::self().srandom(s);
      trace = "";
      randsequence (top)
        top : rand join s1 s3 ;
        s1 : a b ;
        s3 : e { return; } f ;
        a : { trace = {trace, "a"}; } ;
        b : { trace = {trace, "b"}; } ;
        e : { trace = {trace, "e"}; } ;
        f : { trace = {trace, "f"}; } ;
      endsequence
      if (trace.len() != 3) return_errors = return_errors + 1;
    end
    $display("return=%0d", return_errors);
    $finish;
  end
endmodule
