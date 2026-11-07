// A function holding a nonblocking assignment that a static initializer
// reaches keeps its locals in stack cells; the site check reads no local.
module tb;
  int sink = 0;
  function automatic int mark(int v);
    int doubled = v * 2;
    if (v != 0) sink <= doubled;
    return doubled;
  endfunction
  int seed = mark(0);
  initial begin
    void'(mark(3));
    #1 $display("%0d %0d", seed, sink);
    $finish(0);
  end
endmodule
