// ADV-019 threads and frames (IEEE 1800-2009 37.25, 37.39, 37.40, 38.36.1):
// see threads.c and readme.md.
module tb;
  event ev;
  int r, hits;
  task automatic t1(input int a);
    r = a;
    $threads(1);
    #1 $threads(2);
  endtask
  function automatic int fact(int n);
    if (n <= 1) begin
      $threads(3);
      return 1;
    end
    return n * fact(n - 1);
  endfunction
  initial begin
    $threads(0);
    t1(4);
    r = fact(3);
    fork
      #1 $threads(4);
      begin
        #2 $threads(5);
      end
    join
    -> ev;
    #1 $threads(6);
    $display("r=%0d hits=%0d", r, hits);
    $finish;
  end
  always @(ev) hits = hits + 1;
endmodule
