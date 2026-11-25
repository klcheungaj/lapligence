// Decision R14-D2: an array of strings as an `inside` set item is legal
// (SV 11.4.13) but not supported, a parameter included.
module tb;
  parameter string NAMES[2] = '{"a", "b"};
  initial begin
    $display("%0d", "b" inside {NAMES});
    $finish;
  end
endmodule
