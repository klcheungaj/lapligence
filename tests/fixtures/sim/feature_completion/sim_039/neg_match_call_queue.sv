// FIN-001 A3 limit: a sequence match-item call (IEEE 1800-2009 16.11) passing
// a queue by value shares the deferred action frame and its limits; it is
// rejected at the match item.
module tb;
  logic clk = 1'b0, a = 1'b0;
  int q[$];
  function automatic void show(input int x[$]);
    $display("[%0t] size=%0d", $time, x.size());
  endfunction
  sequence s; @(posedge clk) (a, show(q)); endsequence
  c1: cover property (s);
  always #5 clk = ~clk;
  initial begin
    q.push_back(1);
    #7 a = 1'b1;
    #10 $finish;
  end
endmodule
