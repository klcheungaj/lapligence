// Synthetic data: random base64, no real key.
module secret_legacy(a, b);
  input a;
  output b;
`protected
fxJvNYL5VgrggA0EUe98PCVw+gDwF5/Lcb1gih3kSZMi+dBqeJirISMVnGQl0HfK
p5HJF1IxCP9AQf6c7fcg8IeL90/JX80zABQMFIYVYH0YcfLFWgm29NXx
`endprotected
endmodule

module tb;
  reg a;
  wire b;
  secret_legacy u (.a(a), .b(b));
  initial begin
    a = 1'b1;
    #1 $display("b=%b", b);
    $finish;
  end
endmodule
