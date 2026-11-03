// A function reads a queue class property through a local that holds one
// of two handles depending on a run-time choice; the wait cannot name the
// queue whose change it must observe.
module tb;
  class C;
    int q[$];
  endclass
  C h = new;
  C g = new;
  function automatic int pick_size(C a, C b, bit sel);
    C d;
    if (sel) d = a;
    else d = b;
    return d.q.size();
  endfunction
  initial begin
    wait (pick_size(h, g, 1'b1) == 1);
    $display("woke");
  end
  initial #1 h.q.push_back(3);
endmodule
