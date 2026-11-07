// Decision S13-D5 (the run stops, so the expected output is empty): a
// fork-join_none, a nonblocking assignment or a nonblocking event trigger that
// executes under a static declaration initializer stops the run with a
// run-time error. Static initialization runs before any process exists.
// Reaching such a statement without executing it is legal.
//
// IEEE 1800-2009 13.4.4 (SystemVerilog-1800-2009.txt L18993-18997):
//   "Calling a function that tries to schedule an event that cannot become
//   active until after that function returns shall be allowed provided that
//   the thread calling the function is created by an initial procedure,
//   always procedure, or fork block from one of those procedures and in a
//   context in which a side effect is allowed. Implementations shall issue an
//   error either at compile time or run time when these provisions have not
//   been met."
// The clause's own example marks `bit y = watch_for_zero( stack );` illegal.
module tb;
  function automatic void watch();
    fork
      #1 $display("late");
    join_none
  endfunction
  function automatic int start();
    watch();
    return 1;
  endfunction
  int started = start();
  initial begin
    $display("started=%0d", started);
    $finish;
  end
endmodule
