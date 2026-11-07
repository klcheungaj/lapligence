// SIM-022 13.4.4: a nonblocking trigger in a nested call of the initializer.
module tb;
  event e;
  function automatic void fire();
    ->> e;
  endfunction
  function automatic int outer();
    fire();
    return 1;
  endfunction
  int started = outer();
  initial $display("started=%0d", started);
endmodule
