// A negative option.weight passed to a covergroup constructed by a module
// variable's declaration initializer: the run-time error of the same write in
// a process, and the run goes on. Expected values are derived in readme.md.
module tb;
  bit [1:0] x;
  function automatic int note(string s);
    $display("init %s", s);
    return 1;
  endfunction
  covergroup g(int w);
    option.weight = w;
    cp: coverpoint x;
  endgroup
  int first = note("first");
  g a = new(-1);
  int last = note("last");
  initial begin
    $display("initial first=%0d last=%0d weight=%0d", first, last, a.option.weight);
    $finish;
  end
endmodule
