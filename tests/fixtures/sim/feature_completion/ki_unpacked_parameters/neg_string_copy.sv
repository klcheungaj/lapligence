module tb;
  parameter string SA [2] = '{"ab", "cde"};
  string x [2];
  initial begin
    x = SA;
    $display("%s", x[1]);
  end
endmodule
