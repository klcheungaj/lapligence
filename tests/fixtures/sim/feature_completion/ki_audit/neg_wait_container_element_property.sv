// A wait on a class property that a called function reads through a container
// element: the waiter cannot bind the handle, a located limit
// (docs/known_issues.md, waits on storage without a change notification).
module tb;
  class C; int x; endclass
  C q[$];
  C c;
  function automatic int get();
    return q[0].x;
  endfunction
  initial begin
    c = new;
    q.push_back(c);
    fork
      begin wait (get() == 1); $display("woke"); end
      #1 q[0].x = 1;
    join
    $finish;
  end
endmodule
