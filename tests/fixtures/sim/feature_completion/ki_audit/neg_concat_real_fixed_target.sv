// Remaining items of unpacked array concatenations: an item of a run-time
// size into a real fixed target.
module tb;
  real rq[$] = '{1.5};
  real rf[3];
  initial begin
    rf = {rq, 2.5, 3.5};
    $display("%p", rf);
  end
endmodule
