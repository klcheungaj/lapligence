// Decision S13-D8: a function returning an event returns the handle of its
// result variable, which starts as a new synchronization object at each
// call, also in a static function.
//
// IEEE 1800-2009 13.4.1 (SystemVerilog-1800-2009.txt L18796-18797): "The
// function definition shall implicitly declare a variable, internal to the
// function, with the same name as the function. This variable has the same
// type as the function return value." 6.17 (L5966-5967): "If an initial value
// is not specified in the declaration of an event variable, then the variable
// is initialized to a new synchronization object." 6.17 (L5963): "When
// assigned another event variable, both event variables refer to the same
// synchronization object." A static function's result variable is static, so
// the text alone would give one object per static function; industry-standard
// behaviour gives a new one at each call, which this case follows.
module tb;
  function automatic event pass(event f);
    return f;
  endfunction
  function automatic event fdef();
  endfunction
  function event sdef();
  endfunction
  event a, j, e1, e2, s1, s2;
  initial begin
    j = pass(a);
    e1 = fdef();
    e2 = fdef();
    s1 = sdef();
    s2 = sdef();
    $display("j==a %0d e1==null %0d e1==e2 %0d s1==null %0d s1==s2 %0d",
             j == a, e1 == null, e1 == e2, s1 == null, s1 == s2);
  end
  initial #1 forever begin @(j) $display("%0d woke j", $time); end
  initial #1 forever begin @(e2) $display("%0d woke e2", $time); end
  initial #1 forever begin @(s2) $display("%0d woke s2", $time); end
  initial begin
    #2 -> a;
    #1 -> e1;
    #1 -> s1;
    #1 -> e2;
    #1 -> s2;
    #1 $finish;
  end
endmodule
