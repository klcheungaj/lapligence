// A queue insert outside the queue, in a function a declaration initializer
// calls: the index-check stop ends the run at once. Expected values are
// derived in readme.md.
module tb;
  function automatic int note(string s);
    $display("init %s", s);
    return 1;
  endfunction
  function automatic int grow();
    int q[$];
    q.push_back(1);
    q.insert(5, 3);
    $display("grow returns");
    return q.size();
  endfunction
  int first = note("first");
  int size = grow();
  int last = note("last");
  initial begin
    $display("initial first=%0d size=%0d last=%0d", first, size, last);
    $finish;
  end
endmodule
