// Decision S16-D5: rand join first chooses the rule of every joined
// production, in item order, and then interleaves the elements of the chosen
// rules. Each element of a rule (a production item, a code block, an if,
// case or repeat production) is one step and is generated whole, so a
// nonterminal is expanded to depth 1 only. A rule's weight code block runs
// right after its last element. A return aborts the rest of its own
// sequence while the other sequences continue. The clause gives the steps
// only by example (llg choice).
//
// IEEE 1800-2009 18.17.5 (SystemVerilog-1800-2009.txt L31615-31616):
//   "The rand join production control is used to randomly interleave two or
//   more production sequences while maintaining the relative order of each
//   sequence."
// IEEE 1800-2009 18.17.5 (L31671):
//   "At each step, the generator interleaves nonterminal symbols to depth of
//   1."
module tb;
  string trace;
  string choices;
  int order_errors, whole_errors, weight_errors, return_errors, choice_errors;
  bit stop;
  function int pick(string name);
    choices = {choices, name};
    return 1;
  endfunction
  initial begin
    for (int s = 1; s <= 200; s++) begin
      process::self().srandom(s);
      trace = "";
      choices = "";
      stop = (s % 2) == 0;
      randsequence (top)
        top : rand join s1 s2 s3 ;
        s1 : a b := (pick("1")) { trace = {trace, "w"}; } | never := 0 ;
        s2 : pair d := (pick("2")) | never := 0 ;
        s3 : e { if (stop) return; } f ;
        pair : c1 c2 ;
        a : { trace = {trace, "a"}; } ;
        b : { trace = {trace, "b"}; } ;
        c1 : { trace = {trace, "c"}; } ;
        c2 : { trace = {trace, "C"}; } ;
        d : { trace = {trace, "d"}; } ;
        e : { trace = {trace, "e"}; } ;
        f : { trace = {trace, "f"}; } ;
        never : { trace = {trace, "!"}; } ;
      endsequence
      if (choices != "12") choice_errors = choice_errors + 1;
      // Sequence order holds: a before b, pair before d, e before f.
      for (int i = 0; i < trace.len(); i++)
        for (int j = i + 1; j < trace.len(); j++)
          if ((trace[i] == "b" && trace[j] == "a") ||
              (trace[i] == "d" && trace[j] == "c") ||
              (trace[i] == "f" && trace[j] == "e"))
            order_errors = order_errors + 1;
      // The pair element is generated whole: c is always followed by C.
      for (int i = 0; i < trace.len(); i++)
        if (trace[i] == "c" && (i + 1 >= trace.len() || trace[i + 1] != "C"))
          whole_errors = whole_errors + 1;
      // The weight block of s1 follows b at once.
      for (int i = 0; i < trace.len(); i++)
        if (trace[i] == "b" && (i + 1 >= trace.len() || trace[i + 1] != "w"))
          weight_errors = weight_errors + 1;
      // The return drops f and nothing else.
      if (trace.len() != (stop ? 7 : 8)) return_errors = return_errors + 1;
    end
    $display("choice=%0d order=%0d whole=%0d weight=%0d return=%0d",
             choice_errors, order_errors, whole_errors, weight_errors,
             return_errors);
    $finish;
  end
endmodule
