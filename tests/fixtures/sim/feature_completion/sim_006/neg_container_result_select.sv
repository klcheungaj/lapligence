module tb;
  typedef int iq_t[$];
  function automatic iq_t make();
    int q[$];
    q.push_back(4);
    return q;
  endfunction
  int x;
  initial begin
    x = make()[0];
    $display("%0d", x);
  end
endmodule
