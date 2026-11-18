// Decision S22-D9: a $finish executed by a static declaration initializer
// ends the run at once: the rest of the initializer, every later initializer
// and every initial procedure do not run. The final procedures still run.
//
// IEEE 1800-2009 6.8 (SystemVerilog-1800-2009.txt L5286-5287):
//   "Setting the initial value of a static variable as part of the variable
//   declaration (including static class members) shall occur before any
//   initial or always procedures are started"
// IEEE 1800-2009 20.2 (L34119):
//   "The $finish system task causes the simulator to exit and pass control
//   back to the host operating system."
// IEEE 1800-2009 9.2.3 (L11261):
//   "A final procedure executes when simulation ends due to an explicit or
//   implicit call to $finish."
module tb;
  function automatic int note(string s);
    $display("init %s", s);
    return 1;
  endfunction
  function automatic int stop();
    $display("stop called");
    $finish;
    $display("after finish");
    return 7;
  endfunction
  int first = note("first");
  int v = stop();
  int last = note("last");
  initial $display("initial v=%0d last=%0d", v, last);
  final $display("final first=%0d v=%0d last=%0d", first, v, last);
endmodule
