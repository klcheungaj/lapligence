// SIM-022-A03 nearest illegal: a static initializer calls a function whose
// fork-join_none would outlive the call before any process exists.
module tb;
  function automatic int watch();
    fork
      #1 $display("late");
    join_none
    return 1;
  endfunction
  int started = watch();
  initial $display("started=%0d", started);
endmodule
