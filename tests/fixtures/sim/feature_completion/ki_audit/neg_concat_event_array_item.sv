// Remaining items of unpacked array concatenations: a fixed array of events
// as an item.
module tb;
  event e[2];
  event q[$];
  initial begin
    q = {e};
    $display("%0d", q.size());
  end
endmodule
