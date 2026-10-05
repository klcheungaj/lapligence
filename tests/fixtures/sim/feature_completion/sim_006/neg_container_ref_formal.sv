module tb;
  function automatic void grow(ref int q[$]);
    q.push_back(1);
  endfunction
  int q[$];
  initial grow(q);
endmodule
