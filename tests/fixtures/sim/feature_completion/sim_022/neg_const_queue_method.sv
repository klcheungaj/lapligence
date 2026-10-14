// SIM-022-A03 nearest illegal: push_back writes a const queue.
module tb;
  const int limits[$] = '{3, 1, 2};
  initial begin
    $display("size=%0d", limits.size());
    limits.push_back(4);
  end
endmodule
