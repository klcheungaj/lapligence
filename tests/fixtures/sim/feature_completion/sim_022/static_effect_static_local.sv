// SIM-022 13.4.4: a static local's initializer, whose call holds the fork.
module tb;
  function automatic int watch();
    fork
      #1 $display("late");
    join_none
    return 1;
  endfunction
  function automatic int once();
    static int first = watch();
    return first;
  endfunction
  initial $display("once=%0d", once());
endmodule
