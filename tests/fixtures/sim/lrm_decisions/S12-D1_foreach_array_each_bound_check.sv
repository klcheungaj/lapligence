// Decision S12-D1: a `foreach` over a property selected through a call
// evaluates the call at each bound check of an indexed traversal. The text
// has no rule: 12.7.3 (SystemVerilog-1800-2009.txt L18185-18186) says the loop
// "is similar to a repeat-loop that uses the array bounds to specify the
// repeat count instead of an expression", and its grammar takes an array
// identifier.
module tb;
  class C; int q[$]; endclass
  C h;
  int calls = 0;
  function automatic C pick();
    calls++;
    return h;
  endfunction
  initial begin
    h = new;
    h.q.push_back(1);
    h.q.push_back(2);
    foreach (pick().q[i]) $display("%0d", i);
    $display("%0d", calls);
    $finish;
  end
endmodule
