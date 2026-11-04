// S13-D3: an output event formal merges its actual at the return (SV 13.3
// "copy value out at end", 15.5.5.1 "the two become merged"), in a task, a
// void function and a function called in an expression alike.
`timescale 1ns/1ns
module tb;
  class C; event ev; endclass
  C c = new;
  event g, h, h2, h3, h4;
  function automatic int f(output event o);
    o = g;
    return 1;
  endfunction
  function automatic void fv(output event o);
    o = g;
  endfunction
  function automatic int fr(ref event o);
    o = g;
    return 1;
  endfunction
  task automatic t(output event o);
    o = g;
  endtask
  int x, n = 0;
  initial begin
    x = f(h);
    fv(h2);
    x = fr(h3);
    t(h4);
    fork
      #1 -> g;
      begin @(h) $display("%0t h", $time); end
      begin @(h2) $display("%0t h2", $time); end
      begin @(h3) $display("%0t h3", $time); end
      begin @(h4) $display("%0t h4", $time); end
      #5 $display("timeout");
    join_any
    #6 $display("end %0d %0d %0d %0d %0d", h == g, h2 == g, h3 == g, h4 == g, x);
    $finish;
  end
endmodule
