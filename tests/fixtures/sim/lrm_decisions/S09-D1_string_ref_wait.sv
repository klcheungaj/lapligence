// Decision S09-D1: a wait or event control on a string `ref` formal follows
// the variable the actual names: it wakes when that string changes value.
//
// IEEE 1800-2009 9.4.2 (SystemVerilog-1800-2009.txt L11822-11824): "A
//   variable used with the event control can be any one of the integral data
//   types (see 6.11.1) or string. The variable can be either a simple variable
//   or a ref argument (variable passed by reference); it can be a member of an
//   array, associative array, or object (class instance) of the aforementioned
//   types."
// IEEE 1800-2009 13.5.2 (L19138-19141): "When the argument is passed by
//   reference, both the caller and the subroutine share the same
//   representation of the argument; therefore, any changes made to the
//   argument, within either the caller or the subroutine, shall be visible to
//   each other."
//
// llg follows the text (it rejected the recursive task and never woke the
// other before). Writing the same
// value again at time 3 is no change, so `watch` does not resume then.
module tb;
  string word;

  task automatic spell(ref string s, input int n);
    wait (s == $sformatf("w%0d", n));
    $display("spell %0d at %0d", n, $time);
    if (n < 1) spell(s, n + 1);
  endtask

  task automatic watch(ref string s);
    @(s);
    $display("watch %s at %0d", s, $time);
  endtask

  initial begin
    fork
      spell(word, 0);
    join_none
    #1 word = "w0";
    #1 word = "w1";
    fork
      watch(word);
    join_none
    #1 word = "w1";
    #1 word = "w2";
    #1 $finish;
  end
endmodule
