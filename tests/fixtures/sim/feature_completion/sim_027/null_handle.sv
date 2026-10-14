// A memory_name reached through a null class handle: the standard null
// dereference error, before the file is opened or any word is stored.
module tb;
  class holder;
    logic [7:0] cells [0:3];
  endclass
  holder h;
  int i = 1;
  initial begin
    $display("before");
    $writememh("dump.mem", h.cells[i+:2]);
    $display("after");
  end
endmodule
