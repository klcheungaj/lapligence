// $finish in a function an element of an unpacked array pattern initializer
// calls: the run ends at the call. Expected values are derived in readme.md.
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
  int pair[2] = '{note("pattern"), stop()};
  int last = note("last");
  initial $display("initial pair=%0d,%0d last=%0d", pair[0], pair[1], last);
endmodule
