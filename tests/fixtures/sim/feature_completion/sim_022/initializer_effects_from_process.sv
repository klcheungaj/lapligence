// SIM-022-A03 boundary: the same fork and nonblocking assignment are legal
// when a procedure (or an automatic local's initializer inside one) calls the
// function, and an immediate event trigger is legal in a static initializer.
module tb;
  int flag;
  event ping;
  function automatic int watch();
    fork
      #1 $display("late fork");
    join_none
    flag <= 5;
    return 1;
  endfunction
  function automatic int trigger();
    ->ping;
    return 3;
  endfunction
  int triggered = trigger();
  initial begin
    automatic int local_started = watch();
    int started;
    started = watch();
    #2;
    $display("local_started=%0d started=%0d flag=%0d triggered=%0d", local_started, started,
             flag, triggered);
    $finish;
  end
endmodule
