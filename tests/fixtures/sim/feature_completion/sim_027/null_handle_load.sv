// A queue property reached through a null class handle as a load target: the
// standard null dereference error, before any word is stored.
module tb;
  class holder;
    logic [7:0] q [$];
  endclass
  holder h;
  initial begin
    $display("before");
    $readmemh("words.mem", h.q);
    $display("after");
  end
endmodule
