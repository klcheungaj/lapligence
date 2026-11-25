// A slice of an element of a container of containers as an `inside` set item
// is legal (SV 11.4.13) but has no operand form.
module tb;
  string qq[$][$];
  initial begin
    qq.push_back('{"a", "b"});
    $display("%0d", "b" inside {qq[0][0:1]});
    $finish;
  end
endmodule
