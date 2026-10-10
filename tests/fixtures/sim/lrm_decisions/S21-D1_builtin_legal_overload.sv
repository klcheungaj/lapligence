// IEEE 1800-2009 11.11 L16473-16476: "It does not change the meaning of the
// operators for data types where it is legal to apply them." L16562-16563:
// "The assignment operator from a float to a float cannot be overloaded above
// because it is already legal in the three preceding bind statements.
// Similarly, equality and inequality between floats cannot be overloaded."
// Decision (llg reading; the text does not say whether such a declaration is
// an error or merely has no effect): a prototype whose operator is already
// legal for its formal types, with a built-in result assignable to its result
// type, is a compile-time error. Here `+` on two ints returning int.
module tb;
  function automatic int add999(int a, int b);
    return 999;
  endfunction
  bind + function int add999(int, int);
  int x;
  initial begin
    x = 1 + 2;
    $display("%0d", x);
    $finish;
  end
endmodule
