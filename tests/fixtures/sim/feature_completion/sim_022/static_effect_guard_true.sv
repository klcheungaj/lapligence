// SIM-022 13.4.4: the guard is true during the initializer, so the fork runs
// before any process exists.
module tb;
  function automatic int watch(int go);
    if (go > 0) begin
      fork
        #1 $display("late");
      join_none
    end
    return go;
  endfunction
  int started = watch(1);
  initial $display("started=%0d", started);
endmodule
